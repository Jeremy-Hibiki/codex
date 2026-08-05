//! FMSH UKey SDK CMS envelope backends.

use std::sync::Arc;

use anyhow::anyhow;
use anyhow::Result;
use fmsh_ukey_wrapper::Ukey;

use crate::cipher::Cipher;
use crate::cipher::KeyWrap;

fn init_ukey() -> Result<Arc<Ukey>> {
    Ok(Arc::new(
        Ukey::initialize().map_err(|e| anyhow!("UKey init failed: {e}"))?,
    ))
}

/// CMS SM2/SM4 digital envelope through the FMSH UKey SDK.
pub struct UkeyCipher {
    ukey: Arc<Ukey>,
    cert: Option<Vec<u8>>,
    device: Option<String>,
    container: Option<String>,
}

impl UkeyCipher {
    /// `cert` is only needed for encryption; decryption needs `device` /
    /// `container` (both default to the wrapper's env-based resolution).
    pub fn new(
        cert: Option<Vec<u8>>,
        device: Option<String>,
        container: Option<String>,
    ) -> Result<Self> {
        Ok(Self {
            ukey: init_ukey()?,
            cert,
            device,
            container,
        })
    }
}

impl Cipher for UkeyCipher {
    fn mode(&self) -> &'static str {
        "ukey"
    }

    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let cert = self
            .cert
            .as_ref()
            .ok_or_else(|| anyhow!("ukey mode: encryption requires --cert"))?;
        self.ukey
            .encrypt(cert, plaintext)
            .map_err(|e| anyhow!("UKey encrypt failed: {e}"))
    }

    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        self.ukey
            .decrypt(
                ciphertext,
                self.device.as_deref(),
                self.container.as_deref(),
            )
            .map_err(|e| anyhow!("UKey decrypt failed: {e}"))
    }
}

/// UKey-backed [`KeyWrap`]: a symmetric key is wrapped into a CMS envelope.
/// `cert` is needed for wrapping (encryption); `device`/`container` for
/// unwrapping (decryption).
pub struct UkeyKeyWrap {
    ukey: Arc<Ukey>,
    cert: Option<Vec<u8>>,
    device: Option<String>,
    container: Option<String>,
}

impl UkeyKeyWrap {
    pub fn new(
        cert: Option<Vec<u8>>,
        device: Option<String>,
        container: Option<String>,
    ) -> Result<Self> {
        Ok(Self {
            ukey: init_ukey()?,
            cert,
            device,
            container,
        })
    }
}

impl KeyWrap for UkeyKeyWrap {
    fn wrap(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let cert = self
            .cert
            .as_ref()
            .ok_or_else(|| anyhow!("ukey-two-phase mode: wrapping a key requires --cert"))?;
        self.ukey
            .encrypt(cert, plaintext)
            .map_err(|e| anyhow!("UKey key wrap failed: {e}"))
    }

    fn unwrap(&self, envelope: &[u8]) -> Result<Vec<u8>> {
        self.ukey
            .decrypt(envelope, self.device.as_deref(), self.container.as_deref())
            .map_err(|e| anyhow!("UKey key unwrap failed: {e}"))
    }
}
