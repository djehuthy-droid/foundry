// Run from an unmodified kaspanet/kaspad checkout. Generates disposable test
// wallets only; no RPC, existing wallets, or real UTXOs are used.
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"

	"github.com/kaspanet/kaspad/cmd/kaspawallet/keys"
	"github.com/kaspanet/kaspad/cmd/kaspawallet/libkaspawallet"
	"github.com/kaspanet/kaspad/domain/dagconfig"
)

func must(err error) {
	if err != nil {
		panic(err)
	}
}

func main() {
	if len(os.Args) != 2 {
		panic("usage: official-wallet-fixture <empty-directory>")
	}
	dir := os.Args[1]
	const password = "disposable-interop-test-only"
	params := &dagconfig.MainnetParams
	mnemonics := make([]string, 3)
	pubkeys := make([]string, 3)
	for i := range mnemonics {
		var err error
		mnemonics[i], err = libkaspawallet.CreateMnemonic()
		must(err)
		pubkeys[i], err = libkaspawallet.MasterPublicKeyFromMnemonic(params, mnemonics[i], true)
		must(err)
	}
	sorted := append([]string(nil), pubkeys...)
	sort.Strings(sorted)
	for i, mnemonic := range mnemonics {
		wallet, err := keys.NewFileFromMnemonic(params, mnemonic, password)
		must(err)
		wallet.ExtendedPublicKeys = sorted
		wallet.MinimumSignatures = 2
		wallet.CosignerIndex = uint32(sort.SearchStrings(sorted, pubkeys[i]))
		must(wallet.SetPath(params, filepath.Join(dir, fmt.Sprintf("signer%d.keys.json", i+1)), false))
		must(wallet.Save())
	}
	public, err := json.Marshal(map[string]interface{}{
		"publicKeys": sorted, "minimumSignatures": 2, "ecdsa": false,
	})
	must(err)
	must(os.WriteFile(filepath.Join(dir, "public.json"), public, 0600))
	must(os.WriteFile(filepath.Join(dir, "password.txt"), []byte(password), 0600))
}
