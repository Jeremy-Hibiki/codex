//! Hardware-free roundtrips for the pluggable backends.
//!
//! `ukey` and the real UKey-backed key wrap need a GM3000 device, so they are
//! exercised only through the two-phase abstraction with a software key wrap.

use std::sync::Arc;

use anyhow::bail;
use anyhow::Result;
use fmsh_ukey_cipher::Cipher;
use fmsh_ukey_cipher::KeyWrap;
use fmsh_ukey_cipher::LocalCipher;
use fmsh_ukey_cipher::NoopCipher;
use fmsh_ukey_cipher::UkeyTwoPhaseCipher;
use openssl::rand::rand_bytes;
use openssl::symm::Cipher as OsslCipher;
use openssl::symm::Crypter;
use openssl::symm::Mode;

const GCM_IV_LEN: usize = 12;
const GCM_TAG_LEN: usize = 16;

#[test]
fn noop_roundtrip() -> Result<()> {
    let cipher = NoopCipher;
    let data = b"noop payload";
    assert_eq!(cipher.mode(), "noop");
    assert_eq!(cipher.decrypt(&cipher.encrypt(data)?)?, data);
    Ok(())
}

#[test]
fn local_envelope_roundtrip() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let pub_path = tmp.path().join("key.pub.pem");
    let priv_path = tmp.path().join("key.priv.pem");

    let encryptor = LocalCipher::generate_to_files(&pub_path, &priv_path)?;
    assert_eq!(encryptor.mode(), "local");

    let plaintext = b"local envelope payload";
    let envelope = encryptor.encrypt(plaintext)?;
    assert!(envelope.len() >= 32 + GCM_IV_LEN + GCM_TAG_LEN);

    // A decrypt-only cipher (private key alone) must open the envelope.
    let decryptor = LocalCipher::from_priv_file(&priv_path)?;
    assert_eq!(decryptor.decrypt(&envelope)?, plaintext);

    // The public key alone cannot decrypt.
    let pub_only = LocalCipher::from_pub_file(&pub_path)?;
    assert!(pub_only.decrypt(&envelope).is_err());
    Ok(())
}

/// Software key-wrap stand-in for the UKey CMS envelope.
struct AesKeyWrap {
    key: [u8; 32],
}

impl AesKeyWrap {
    fn new() -> Result<Self> {
        let mut key = [0u8; 32];
        rand_bytes(&mut key)?;
        Ok(Self { key })
    }
}

impl KeyWrap for AesKeyWrap {
    fn wrap(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut iv = [0u8; GCM_IV_LEN];
        rand_bytes(&mut iv)?;
        let mut crypter = Crypter::new(
            OsslCipher::aes_256_gcm(),
            Mode::Encrypt,
            &self.key,
            Some(&iv),
        )?;
        let mut encrypted = vec![0u8; plaintext.len() + GCM_TAG_LEN];
        let mut n = crypter.update(plaintext, &mut encrypted)?;
        n += crypter.finalize(&mut encrypted[n..])?;
        let mut tag = [0u8; GCM_TAG_LEN];
        crypter.get_tag(&mut tag)?;
        encrypted.truncate(n);

        let mut out = Vec::with_capacity(GCM_IV_LEN + GCM_TAG_LEN + encrypted.len());
        out.extend_from_slice(&iv);
        out.extend_from_slice(&tag);
        out.extend_from_slice(&encrypted);
        Ok(out)
    }

    fn unwrap(&self, envelope: &[u8]) -> Result<Vec<u8>> {
        if envelope.len() < GCM_IV_LEN + GCM_TAG_LEN {
            bail!("wrapped key too short");
        }
        let (iv, rest) = envelope.split_at(GCM_IV_LEN);
        let (tag, encrypted) = rest.split_at(GCM_TAG_LEN);
        let mut crypter = Crypter::new(
            OsslCipher::aes_256_gcm(),
            Mode::Decrypt,
            &self.key,
            Some(iv),
        )?;
        crypter.set_tag(tag)?;
        let mut plaintext = vec![0u8; encrypted.len()];
        let mut n = crypter.update(encrypted, &mut plaintext)?;
        n += crypter.finalize(&mut plaintext[n..])?;
        plaintext.truncate(n);
        Ok(plaintext)
    }
}

#[test]
fn ukey_two_phase_roundtrip() -> Result<()> {
    let wrap = Arc::new(AesKeyWrap::new()?);

    // Encryption side: encrypt several files, then export the wrapped key.
    let encryptor = UkeyTwoPhaseCipher::new(wrap.clone());
    let payloads: Vec<Vec<u8>> = (0..8)
        .map(|i| format!("two-phase payload {i}").into_bytes())
        .collect();
    let envelopes: Vec<Vec<u8>> = payloads
        .iter()
        .map(|p| encryptor.encrypt(p))
        .collect::<Result<_>>()?;
    let wrapped_key = encryptor.wrap_key()?;
    assert!(encryptor.key_ready());

    // Decryption side: fresh cipher, one unwrap, then all files decrypt
    // without any further key-wrap calls.
    let decryptor = UkeyTwoPhaseCipher::new(wrap.clone());
    assert!(!decryptor.key_ready());
    assert!(decryptor.decrypt(&envelopes[0]).is_err());

    decryptor.unwrap_key(&wrapped_key)?;
    assert!(decryptor.key_ready());
    for (envelope, payload) in envelopes.iter().zip(&payloads) {
        assert_eq!(decryptor.decrypt(envelope)?, *payload);
    }
    Ok(())
}

#[test]
fn ukey_two_phase_decrypt_is_parallel_safe() -> Result<()> {
    let wrap = Arc::new(AesKeyWrap::new()?);
    let encryptor = UkeyTwoPhaseCipher::new(wrap.clone());
    let payloads: Vec<Vec<u8>> = (0..16)
        .map(|i| format!("parallel payload {i}").into_bytes())
        .collect();
    let envelopes: Vec<Vec<u8>> = payloads
        .iter()
        .map(|p| encryptor.encrypt(p))
        .collect::<Result<_>>()?;
    let wrapped_key = encryptor.wrap_key()?;

    let decryptor = Arc::new(UkeyTwoPhaseCipher::new(wrap.clone()));
    decryptor.unwrap_key(&wrapped_key)?;

    let handles: Vec<_> = envelopes
        .iter()
        .zip(&payloads)
        .map(|(envelope, expected)| {
            let cipher = decryptor.clone();
            let envelope = envelope.clone();
            let expected = expected.clone();
            std::thread::spawn(move || {
                let plaintext = cipher.decrypt(&envelope)?;
                assert_eq!(plaintext, expected);
                Ok::<(), anyhow::Error>(())
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("decrypt thread panicked")?;
    }
    Ok(())
}

#[test]
fn ukey_two_phase_rejects_wrong_key_length() -> Result<()> {
    struct IdentityWrap;

    impl KeyWrap for IdentityWrap {
        fn wrap(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
            Ok(plaintext.to_vec())
        }

        fn unwrap(&self, envelope: &[u8]) -> Result<Vec<u8>> {
            Ok(envelope.to_vec())
        }
    }

    let cipher = UkeyTwoPhaseCipher::new(Arc::new(IdentityWrap));
    let wrapped_short = b"not-a-32-byte-key";
    assert!(cipher.unwrap_key(wrapped_short).is_err());
    assert!(!cipher.key_ready());
    Ok(())
}
