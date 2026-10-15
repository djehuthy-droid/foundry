//! Kaspa wallet options retained alongside upstream EVM wallet options.

use clap::Parser;
use foundry_config::{Config, IgraKaspaWalletConfig};
use serde::Serialize;

#[derive(Clone, Debug, Default, Serialize, Parser)]
#[command(next_help_heading = "Wallet options - kaspa", about = None, long_about = None)]
pub struct KaspaWalletOpts {
    /// Use the provided Kaspa private key.
    #[arg(
        long = "private-key-kaspa",
        env = "KASPA_PRIVATE_KEY",
        value_name = "RAW_PRIVATE_KEY",
        hide_env_values = true
    )]
    pub private_key_kaspa: Option<String>,

    /// Use the Kaspa mnemonic phrase.
    #[arg(
        long = "mnemonic-kaspa",
        env = "KASPA_MNEMONIC",
        value_name = "MNEMONIC",
        hide_env_values = true
    )]
    pub mnemonic_kaspa: Option<String>,

    /// Use a BIP39 passphrase for the Kaspa mnemonic.
    #[arg(
        long = "mnemonic-passphrase-kaspa",
        env = "KASPA_MNEMONIC_PASSPHRASE",
        value_name = "PASSPHRASE",
        hide_env_values = true
    )]
    pub mnemonic_passphrase_kaspa: Option<String>,

    /// Kaspa mnemonic derivation path override.
    #[arg(
        long = "mnemonic-derivation-path-kaspa",
        env = "KASPA_MNEMONIC_DERIVATION_PATH",
        value_name = "PATH"
    )]
    pub mnemonic_derivation_path_kaspa: Option<String>,

    /// Kaspa mnemonic index override.
    #[arg(long = "mnemonic-index-kaspa", env = "KASPA_MNEMONIC_INDEX", value_name = "INDEX")]
    pub mnemonic_index_kaspa: Option<u32>,

    /// Use a Kaspa keystore file.
    #[arg(long = "keystore-kaspa", env = "KASPA_KEYSTORE", value_name = "PATH")]
    pub keystore_kaspa: Option<String>,

    /// Use a Kaspa keystore account alias.
    #[arg(
        long = "keystore-account-kaspa",
        env = "KASPA_KEYSTORE_ACCOUNT",
        value_name = "ACCOUNT_NAME"
    )]
    pub keystore_account_kaspa: Option<String>,

    /// Kaspa keystore password.
    #[arg(
        long = "password-kaspa",
        env = "KASPA_PASSWORD",
        value_name = "PASSWORD",
        hide_env_values = true
    )]
    pub password_kaspa: Option<String>,
}

impl KaspaWalletOpts {
    /// Returns true when explicit Kaspa signer source was provided.
    pub fn is_set(&self) -> bool {
        self.private_key_kaspa.is_some()
            || self.mnemonic_kaspa.is_some()
            || self.mnemonic_passphrase_kaspa.is_some()
            || self.mnemonic_derivation_path_kaspa.is_some()
            || self.mnemonic_index_kaspa.is_some()
            || self.keystore_kaspa.is_some()
            || self.keystore_account_kaspa.is_some()
            || self.password_kaspa.is_some()
    }

    /// Converts CLI/env Kaspa options to config shape.
    pub fn as_config(&self) -> IgraKaspaWalletConfig {
        IgraKaspaWalletConfig {
            private_key: self.private_key_kaspa.clone(),
            mnemonic: self.mnemonic_kaspa.clone(),
            mnemonic_passphrase: self.mnemonic_passphrase_kaspa.clone(),
            mnemonic_derivation_path: self.mnemonic_derivation_path_kaspa.clone(),
            mnemonic_index: self.mnemonic_index_kaspa,
            keystore: self.keystore_kaspa.clone(),
            keystore_account: self.keystore_account_kaspa.clone(),
            password: self.password_kaspa.clone(),
        }
    }

    /// Apply explicit Kaspa options, then configured custody, then the EVM fallback.
    pub fn apply_single(&self, config: &mut Config, wallet: &foundry_wallets::WalletOpts) {
        self.apply(config, single_fallback(wallet));
    }

    /// Apply the same precedence for script multi-wallet options.
    pub fn apply_multi(&self, config: &mut Config, wallet: &foundry_wallets::MultiWalletOpts) {
        self.apply(config, multi_fallback(wallet));
    }

    fn apply(&self, config: &mut Config, fallback: Option<IgraKaspaWalletConfig>) {
        if self.is_set() {
            config.igra.kaspa_wallet = self.as_config();
        } else if config.igra.kaspa_wallet.is_empty()
            && let Some(fallback) = fallback
        {
            tracing::warn!(
                "IGRA Kaspa wallet fallback reuses EVM key material; prefer explicit Kaspa wallet options"
            );
            config.igra.kaspa_wallet = fallback;
        }
    }
}

fn single_fallback(wallet: &foundry_wallets::WalletOpts) -> Option<IgraKaspaWalletConfig> {
    if let Some(private_key) = wallet.raw.private_key.clone() {
        return Some(IgraKaspaWalletConfig {
            private_key: Some(private_key),
            ..Default::default()
        });
    }

    if let Some(mnemonic) = wallet.raw.mnemonic.clone() {
        return Some(IgraKaspaWalletConfig {
            mnemonic: Some(mnemonic),
            mnemonic_passphrase: wallet.raw.mnemonic_passphrase.clone(),
            mnemonic_derivation_path: wallet.raw.hd_path.clone(),
            mnemonic_index: Some(wallet.raw.mnemonic_index),
            ..Default::default()
        });
    }

    if wallet.keystore_path.is_some() || wallet.keystore_account_name.is_some() {
        return Some(IgraKaspaWalletConfig {
            keystore: wallet.keystore_path.clone(),
            keystore_account: wallet.keystore_account_name.clone(),
            password: wallet.keystore_password.clone(),
            ..Default::default()
        });
    }

    None
}

fn multi_fallback(
    wallet: &foundry_wallets::MultiWalletOpts,
) -> Option<foundry_config::IgraKaspaWalletConfig> {
    if let Some(private_key) = wallet.private_key.clone().or_else(|| {
        wallet.private_keys.as_ref().and_then(|private_keys| private_keys.first().cloned())
    }) {
        return Some(foundry_config::IgraKaspaWalletConfig {
            private_key: Some(private_key),
            ..Default::default()
        });
    }

    if let Some(mnemonic) =
        wallet.mnemonics.as_ref().and_then(|mnemonics| mnemonics.first().cloned())
    {
        let mnemonic_passphrase =
            wallet.mnemonic_passphrases.as_ref().and_then(|phrases| phrases.first().cloned());
        let mnemonic_derivation_path =
            wallet.hd_paths.as_ref().and_then(|paths| paths.first().cloned());
        let mnemonic_index = wallet
            .mnemonic_indexes
            .as_ref()
            .and_then(|indexes| indexes.first().copied())
            .or(Some(0));

        return Some(foundry_config::IgraKaspaWalletConfig {
            mnemonic: Some(mnemonic),
            mnemonic_passphrase,
            mnemonic_derivation_path,
            mnemonic_index,
            ..Default::default()
        });
    }

    let keystore = wallet.keystore_paths.as_ref().and_then(|paths| paths.first().cloned());
    let keystore_account =
        wallet.keystore_account_names.as_ref().and_then(|names| names.first().cloned());
    if keystore.is_some() || keystore_account.is_some() {
        let password =
            wallet.keystore_passwords.as_ref().and_then(|passwords| passwords.first().cloned());
        return Some(foundry_config::IgraKaspaWalletConfig {
            keystore,
            keystore_account,
            password,
            ..Default::default()
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opts::EthereumOpts;

    #[test]
    fn igra_explicit_cli_key_overrides_evm_fallback() {
        let opts = EthereumOpts::parse_from([
            "cast",
            "--private-key",
            "evm-test-key",
            "--private-key-kaspa",
            "kaspa-test-key",
        ]);
        let mut config = Config::default();
        opts.kaspa.apply_single(&mut config, &opts.wallet);
        assert_eq!(config.igra.kaspa_wallet.private_key.as_deref(), Some("kaspa-test-key"));
    }

    #[test]
    fn igra_existing_config_precedes_evm_fallback() {
        let opts = EthereumOpts::parse_from(["cast", "--private-key", "evm-test-key"]);
        let mut config = Config::default();
        config.igra.kaspa_wallet.private_key = Some("configured-kaspa-key".into());
        opts.kaspa.apply_single(&mut config, &opts.wallet);
        assert_eq!(config.igra.kaspa_wallet.private_key.as_deref(), Some("configured-kaspa-key"));
    }

    #[test]
    fn igra_multi_wallet_fallback_is_preserved() {
        let wallets = foundry_wallets::MultiWalletOpts::parse_from([
            "script",
            "--private-key",
            "evm-test-key",
        ]);
        let mut config = Config::default();
        KaspaWalletOpts::default().apply_multi(&mut config, &wallets);
        assert_eq!(config.igra.kaspa_wallet.private_key.as_deref(), Some("evm-test-key"));
    }
}
