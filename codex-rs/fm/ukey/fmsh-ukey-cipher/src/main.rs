//! fmsh-ukey-cipher — pluggable file encryption/decryption CLI.
//!
//! Four backends behind one command surface:
//!
//! ```text
//! noop          identity; decrypt output is the input minus .enc
//! local         X25519 digital envelope (local key pair)
//! ukey          CMS SM2/SM4 envelope via the FMSH UKey SDK
//! ukey-two-phase  Phase 1: UKey unwraps one AES-256-GCM key. Phase 2: all other files decrypt in software.
//! ```

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;
use fmsh_ukey_cipher::append_enc;
use fmsh_ukey_cipher::strip_enc;
use fmsh_ukey_cipher::Cipher;
use fmsh_ukey_cipher::LocalCipher;
use fmsh_ukey_cipher::NoopCipher;
use fmsh_ukey_cipher::UkeyCipher;
use fmsh_ukey_cipher::UkeyKeyWrap;
use fmsh_ukey_cipher::UkeyTwoPhaseCipher;

#[derive(Parser)]
#[command(
    name = "fmsh-ukey-cipher",
    version,
    about = "Pluggable file encryption CLI (noop / local / ukey / ukey-two-phase)"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Encrypt a file or directory.
    Encrypt(FileArgs),
    /// Decrypt a file or directory.
    Decrypt(FileArgs),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Mode {
    /// Identity transform; decrypt output is the input minus `.enc`.
    Noop,
    /// X25519 digital envelope with a local key pair.
    Local,
    /// CMS SM2/SM4 envelope through the UKey SDK.
    Ukey,
    /// Phase 1: UKey unwraps one symmetric key; phase 2: all files use software AES-256-GCM.
    UkeyTwoPhase,
}

impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Mode::Noop => "noop",
            Mode::Local => "local",
            Mode::Ukey => "ukey",
            Mode::UkeyTwoPhase => "ukey-two-phase",
        }
    }
}

#[derive(clap::Args)]
struct FileArgs {
    /// Encryption backend.
    #[arg(long, value_enum, default_value = "ukey")]
    mode: Mode,

    /// Certificate file (PEM/DER) — required to encrypt in ukey / ukey-two-phase mode.
    #[arg(long)]
    cert: Option<PathBuf>,

    /// X25519 public key (PEM) — required for local encryption.
    #[arg(long)]
    local_pubkey: Option<PathBuf>,

    /// X25519 private key (PEM) — required for local decryption.
    #[arg(long)]
    local_privkey: Option<PathBuf>,

    /// UKey device name (defaults to the first enumerated device).
    #[arg(long)]
    device: Option<String>,

    /// UKey container name (defaults to $FMSH_UKEY_CONTAINER).
    #[arg(long)]
    container: Option<String>,

    /// Wrapped key envelope — required for ukey-two-phase mode.
    ///
    /// Encrypt: output path for the newly wrapped key. Decrypt: input path of
    /// the wrapped key, unwrapped once via UKey (phase 1) and kept for the run.
    #[arg(long)]
    key_envelope: Option<PathBuf>,

    /// Allow overwriting an existing --key-envelope file during encryption.
    #[arg(long)]
    force: bool,

    /// Input file or directory.
    #[arg(long)]
    input: PathBuf,

    /// Output file or directory. For a single input file the default is
    /// `<input>.enc` (encrypt) or `<input>` minus `.enc` (decrypt).
    #[arg(long)]
    output: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let result = run(cli);
    // The SDK finalizer must run before process exit to avoid provider
    // destructor ordering crashes; it is safe when never initialized.
    fmsh_ukey_wrapper::Ukey::finalize();
    result
}

fn run(cli: Cli) -> Result<()> {
    match cli.cmd {
        Cmd::Encrypt(args) => run_encrypt(args),
        Cmd::Decrypt(args) => run_decrypt(args),
    }
}

fn run_encrypt(args: FileArgs) -> Result<()> {
    match args.mode {
        Mode::UkeyTwoPhase => {
            let cipher = build_two_phase_cipher(&args, true)?;
            ensure_key_envelope(&cipher, &args)?;
            process_paths(&args, &cipher, false)
        }
        _ => {
            let cipher = build_cipher(args.mode, &args, true)?;
            process_paths(&args, cipher.as_ref(), false)
        }
    }
}

fn run_decrypt(args: FileArgs) -> Result<()> {
    match args.mode {
        Mode::UkeyTwoPhase => {
            let cipher = build_two_phase_cipher(&args, false)?;
            load_key(&cipher, &args)?;
            process_paths(&args, &cipher, true)
        }
        _ => {
            let cipher = build_cipher(args.mode, &args, false)?;
            process_paths(&args, cipher.as_ref(), true)
        }
    }
}

fn build_cipher(mode: Mode, args: &FileArgs, encrypt: bool) -> Result<Arc<dyn Cipher>> {
    match mode {
        Mode::Noop => Ok(Arc::new(NoopCipher)),
        Mode::Local => {
            if encrypt {
                let pub_path = args
                    .local_pubkey
                    .as_ref()
                    .ok_or_else(|| anyhow!("local mode encryption requires --local-pubkey"))?;
                let priv_path = args.local_privkey.as_ref().ok_or_else(|| {
                    anyhow!(
                        "local mode encryption requires --local-privkey (generated when missing)"
                    )
                })?;
                if !pub_path.exists() || !priv_path.exists() {
                    for path in [pub_path, priv_path] {
                        if let Some(parent) = path.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                    }
                    LocalCipher::generate_to_files(pub_path, priv_path)?;
                    println!("Generated local key pair: {}", pub_path.display());
                }
                Ok(Arc::new(LocalCipher::from_key_files(pub_path, priv_path)?))
            } else {
                let priv_path = args
                    .local_privkey
                    .as_ref()
                    .ok_or_else(|| anyhow!("local mode decryption requires --local-privkey"))?;
                let cipher = match &args.local_pubkey {
                    Some(pub_path) => LocalCipher::from_key_files(pub_path, priv_path)?,
                    None => LocalCipher::from_priv_file(priv_path)?,
                };
                Ok(Arc::new(cipher))
            }
        }
        Mode::Ukey => {
            let cert = read_cert(args, encrypt)?;
            Ok(Arc::new(UkeyCipher::new(
                cert,
                args.device.clone(),
                args.container.clone(),
            )?))
        }
        Mode::UkeyTwoPhase => unreachable!("ukey-two-phase is handled by build_two_phase_cipher"),
    }
}

fn build_two_phase_cipher(args: &FileArgs, encrypt: bool) -> Result<UkeyTwoPhaseCipher> {
    let cert = read_cert(args, encrypt)?;
    let key_wrap = Arc::new(UkeyKeyWrap::new(
        cert,
        args.device.clone(),
        args.container.clone(),
    )?);
    Ok(UkeyTwoPhaseCipher::new(key_wrap))
}

fn read_cert(args: &FileArgs, encrypt: bool) -> Result<Option<Vec<u8>>> {
    if !encrypt {
        return Ok(None);
    }
    let path = args
        .cert
        .as_ref()
        .ok_or_else(|| anyhow!("mode {} encryption requires --cert", args.mode.as_str()))?;
    let cert = std::fs::read(path).with_context(|| format!("reading cert {}", path.display()))?;
    Ok(Some(cert))
}

fn ensure_key_envelope(cipher: &UkeyTwoPhaseCipher, args: &FileArgs) -> Result<()> {
    let path = args
        .key_envelope
        .as_ref()
        .ok_or_else(|| anyhow!("ukey-two-phase mode requires --key-envelope"))?;
    if path.exists() && !args.force {
        bail!(
            "--key-envelope {} already exists; pass --force to replace it \
             (files encrypted with the old key would become undecryptable)",
            path.display()
        );
    }
    let wrapped = cipher.wrap_key()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, &wrapped)
        .with_context(|| format!("writing key envelope {}", path.display()))?;
    println!("Wrote key envelope: {}", path.display());
    Ok(())
}

fn load_key(cipher: &UkeyTwoPhaseCipher, args: &FileArgs) -> Result<()> {
    let path = args
        .key_envelope
        .as_ref()
        .ok_or_else(|| anyhow!("ukey-two-phase mode requires --key-envelope"))?;
    let wrapped =
        std::fs::read(path).with_context(|| format!("reading key envelope {}", path.display()))?;
    // This is the single UKey hardware call of the whole decrypt run.
    cipher
        .unwrap_key(&wrapped)
        .with_context(|| format!("unwrapping key {}", path.display()))?;
    println!("Loaded key (one UKey call): {}", path.display());
    Ok(())
}

fn process_paths(args: &FileArgs, cipher: &dyn Cipher, decrypt: bool) -> Result<()> {
    let input = &args.input;
    if !input.exists() {
        bail!("input does not exist: {}", input.display());
    }
    if input.is_dir() {
        let output = args
            .output
            .as_ref()
            .ok_or_else(|| anyhow!("--output directory is required when input is a directory"))?;
        let key_envelope = if decrypt && args.mode == Mode::UkeyTwoPhase {
            args.key_envelope
                .as_ref()
                .and_then(|p| std::fs::canonicalize(p).ok())
        } else {
            None
        };
        let mut count = 0usize;
        for entry in walk_files(input)? {
            let rel = entry
                .strip_prefix(input)
                .expect("walked path is under input");
            let rel_out = if decrypt {
                match strip_enc(rel) {
                    Some(stripped) => stripped,
                    None => continue, // only .enc files are decrypted
                }
            } else {
                if strip_enc(rel).is_some() {
                    continue; // skip already-encrypted files (idempotent batch)
                }
                append_enc(rel)
            };
            if key_envelope
                .as_ref()
                .is_some_and(|canon| std::fs::canonicalize(&entry).ok().as_ref() == Some(canon))
            {
                continue; // never treat the key envelope as a data file
            }
            let dst = output.join(rel_out);
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent)?;
            }
            cipher_file(cipher, &entry, &dst, decrypt)?;
            count += 1;
        }
        println!(
            "{} {} file(s) into {}",
            if decrypt { "Decrypted" } else { "Encrypted" },
            count,
            output.display()
        );
        Ok(())
    } else {
        let dst = match &args.output {
            Some(output) => output.clone(),
            None => default_output(input, decrypt)?,
        };
        cipher_file(cipher, input, &dst, decrypt)?;
        println!(
            "{}: {}",
            if decrypt { "Decrypted" } else { "Encrypted" },
            dst.display()
        );
        Ok(())
    }
}

fn default_output(input: &Path, decrypt: bool) -> Result<PathBuf> {
    if decrypt {
        strip_enc(input).ok_or_else(|| {
            anyhow!(
                "--output is required: {} has no .enc suffix",
                input.display()
            )
        })
    } else {
        Ok(append_enc(input))
    }
}

fn cipher_file(cipher: &dyn Cipher, src: &Path, dst: &Path, decrypt: bool) -> Result<()> {
    if src == dst {
        bail!("output {} would overwrite input", dst.display());
    }
    let data = std::fs::read(src).with_context(|| format!("reading input {}", src.display()))?;
    let out = if decrypt {
        cipher.decrypt(&data)
    } else {
        cipher.encrypt(&data)
    }
    .with_context(|| {
        format!(
            "{} {}",
            if decrypt { "decrypting" } else { "encrypting" },
            src.display()
        )
    })?;
    std::fs::write(dst, &out).with_context(|| format!("writing output {}", dst.display()))?;
    Ok(())
}

fn walk_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk_inner(dir, &mut out)?;
    out.sort();
    Ok(out)
}

#[allow(clippy::only_used_in_recursion)]
fn walk_inner(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            walk_inner(&path, out)?;
        } else if file_type.is_file() {
            out.push(path);
        }
    }
    Ok(())
}
