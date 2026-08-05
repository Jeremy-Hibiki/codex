# fmsh-ukey-cipher

Pluggable file encryption/decryption library and CLI. Everything goes through
the [`Cipher`] abstraction, so switching backends never touches crypto code.

## Modes

| mode | backend | hardware |
|---|---|---|
| `noop` | identity; decrypt output is the input minus `.enc` | none |
| `local` | X25519 digital envelope (ECDH + HKDF-SHA256 + AES-256-GCM) | none |
| `ukey` | CMS SM2/SM4 envelope via the FMSH UKey SDK | UKey for decrypt only |
| `ukey-two-phase` | phase 1: UKey unwraps one AES-256-GCM key; phase 2: every other file decrypts in software | one UKey call per run |

`ukey-two-phase` exists because each UKey decrypt is a slow hardware roundtrip
and the SDK serializes device access. A decrypt run therefore does exactly
one UKey call: it unwraps the key envelope (a CMS envelope containing
32 random bytes) and keeps the key in process memory. Every `.enc` data file
is then AES-256-GCM decrypted in software and can be processed concurrently.

Data-file format (fixed sizes):

```text
[magic "FMSH2PH1" 8][iv 12][auth_tag 16][ciphertext]
```

## CLI

```bash
cargo build -p fmsh-ukey-cipher
bin=target/debug/fmsh-ukey-cipher
```

Single file or whole directory (recursive, preserves relative paths):

```bash
# NOOP — decrypt output is the input minus .enc
"$bin" encrypt --mode noop --input secret.txt          # -> secret.txt.enc
"$bin" decrypt --mode noop --input secret.txt.enc      # -> secret.txt

# Local X25519 envelope (key pair auto-generated when missing)
"$bin" encrypt --mode local \
  --local-pubkey keys/enc.pub.pem --local-privkey keys/enc.priv.pem \
  --input secret.txt --output secret.enc
"$bin" decrypt --mode local --local-privkey keys/enc.priv.pem \
  --input secret.enc --output secret.txt

# UKey CMS envelope
FMSH_UKEY_PROVIDER=.../libgm3000.1.0.so "$bin" encrypt --mode ukey \
  --cert cert.pem --input secret.txt --output secret.enc
FMSH_UKEY_PROVIDER=.../libgm3000.1.0.so "$bin" decrypt --mode ukey \
  --input secret.enc --output secret.txt

# UKey two-phase: one hardware call unwraps the key (phase 1), then all files decrypt
# with software AES-256-GCM
FMSH_UKEY_PROVIDER=.../libgm3000.1.0.so "$bin" encrypt --mode ukey-two-phase \
  --cert cert.pem --key-envelope key.enc \
  --input docs/ --output docs-enc/
FMSH_UKEY_PROVIDER=.../libgm3000.1.0.so "$bin" decrypt --mode ukey-two-phase \
  --key-envelope key.enc \
  --input docs-enc/ --output docs-dec/
```

Directory mode skips existing `.enc` files when encrypting and only processes
`*.enc` files when decrypting. When decrypting with `ukey-two-phase`, the
key envelope is unwrapped first and never treated as a data file.
Encryption refuses to overwrite an existing `--key-envelope` unless
`--force` is passed (old files would become undecryptable).

## Library

```rust
use fmsh_ukey_cipher::{Cipher, LocalCipher, UkeyCipher, UkeyTwoPhaseCipher};

let cipher = LocalCipher::generate_to_files(pub_path, priv_path)?;
let envelope = cipher.encrypt(b"hello")?;
assert_eq!(cipher.decrypt(&envelope)?, b"hello");
```

`UkeyTwoPhaseCipher` takes any [`KeyWrap`]; the real backend is `UkeyKeyWrap`
(CMS envelope through the SDK), and the trait keeps the two-phase logic
testable without hardware.

## Testing

Hardware-free tests (`noop`, `local`, and `ukey-two-phase` with a software key
wrap, including parallel decryption):

```bash
cargo test -p fmsh-ukey-cipher
```

The `ukey` mode and real `UkeyKeyWrap` need a GM3000 device and follow the
usual `FMSH_UKEY_PROVIDER` / `FMSH_UKEY_CONTAINER` conventions.
