#!/usr/bin/env python3
"""Offline-only compatibility test. Uses disposable wallets and a nonexistent UTXO."""
import argparse
import json
import os
from pathlib import Path
import subprocess


def run(args, **kwargs):
    result = subprocess.run([str(x) for x in args], text=True, capture_output=True, **kwargs)
    if result.returncode:
        raise RuntimeError(result.stderr or result.stdout or "Command failed without diagnostics")
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cast", type=Path, required=True)
    parser.add_argument("--wallet", type=Path, required=True)
    parser.add_argument("--kaspad-source", type=Path, required=True)
    parser.add_argument("--work-dir", type=Path, required=True)
    args = parser.parse_args()
    args.cast = args.cast.resolve()
    args.wallet = args.wallet.resolve()
    args.kaspad_source = args.kaspad_source.resolve()
    work = args.work_dir.resolve()
    os.umask(0o077)
    work.mkdir(mode=0o700)  # Never reuse existing wallets/artifacts.
    helper = Path(__file__).with_name("official-wallet-fixture.go").resolve()
    run(["go", "run", helper, work], cwd=args.kaspad_source)
    cast = [args.cast, "igra"]

    address = json.loads(run(cast + ["derive-msig-address", "--network", "mainnet",
                                    "--path", "m/0/0/1", "--keys-file", work / "public.json"]))
    custody_args = ["--exit-address", address["address"]]
    public = json.loads((work / "public.json").read_text())
    build_input = {
        "locking_utxos": [{"transaction_id": "11" * 32, "index": 0,
                           "amount_sompi": 300010000, "address": address["address"],
                           "script_public_key": address["script_public_key"],
                           "derivation_path": "m/0/0/1"}],
        "exits": [{"message_id": "aa" * 32, "amount_sompi": 300000000,
                   "recipient": "kaspa:qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqkx9awp4e"}],
        "fee_sompi": 10000,
        "multisig": {"minimum_signatures": 2,
                     "extended_public_keys": public["publicKeys"], "ecdsa": False},
    }
    # Exercise batching and custody change, not just a single signing input.
    build_input["locking_utxos"][0]["amount_sompi"] = 400010000
    build_input["locking_utxos"].append({**build_input["locking_utxos"][0],
                                        "transaction_id": "22" * 32})
    build_input["exits"].append({**build_input["exits"][0], "message_id": "bb" * 32})
    build_input["change"] = {"derivation_path": "m/0/0/1", "amount_sompi": 200000000,
                             "address": address["address"]}
    build_input["fee_sompi"] = 20000
    (work / "input.json").write_text(json.dumps(build_input, indent=2))
    manifest = work / "unsigned.json"
    unsigned = work / "unsigned.hex"
    run(cast + ["build-exit", "--network", "mainnet", "--tx-id-prefix", "97b1",
                "--lane-id", "97b10000", "--input", work / "input.json",
                "--out-json", manifest, "--out-hex", unsigned] + custody_args)
    verify = cast + ["verify-exit", "--manifest", manifest] + custody_args
    run(verify + ["--hex", unsigned])
    declared = json.loads(manifest.read_text())
    assert declared["exit_address"] == address["address"]
    other_address = json.loads(run(cast + ["derive-msig-address", "--network", "mainnet",
                                          "--path", "m/0/0/2", "--keys-file", work / "public.json"]))
    wrong_pin = subprocess.run([str(x) for x in cast + ["verify-exit", "--manifest", manifest,
                               "--hex", unsigned, "--exit-address", other_address["address"]]],
                               text=True, capture_output=True)
    if wrong_pin.returncode == 0 or "does not match configured custody address" not in wrong_pin.stderr:
        raise RuntimeError("Unexpected custody-pin validation result: " + wrong_pin.stderr)

    # Actual stock CLI invocation; success here is NOT proof of a valid v1 signature.
    go_signed = run([args.wallet, "sign", "--keys-file", work / "signer1.keys.json",
                     "--password", "disposable-interop-test-only", "--transaction-file", unsigned])
    (work / "go-signed.hex").write_text(go_signed)
    go_check = subprocess.run([str(x) for x in verify + ["--hex", work / "go-signed.hex",
                                                       "--allow-signatures"]],
                              text=True, capture_output=True)
    (work / "go-verification.txt").write_text(go_check.stdout + go_check.stderr)
    if go_check.returncode == 0:
        raise RuntimeError("Official wallet behavior changed: review compatibility before updating expectations")
    if "invalid partial signature" not in go_check.stderr:
        raise RuntimeError("Unexpected official-wallet rejection: " + go_check.stderr)
    inspection = lambda path: json.loads(run(cast + ["inspect-exit", "--network", "mainnet", "--hex", path]))
    unsigned_inspection = inspection(unsigned)
    go_inspection = inspection(work / "go-signed.hex")

    # Each signer receives the SAME unsigned artifact; no sequential key exchange.
    responses = []
    for i in range(1, 4):
        response = work / f"cast-signed-{i}.hex"
        run(cast + ["sign-exit", "--manifest", manifest, "--hex", unsigned,
                    "--keys-file", work / f"signer{i}.keys.json",
                    "--keys-password-file", work / "password.txt", "--out-hex", response] + custody_args)
        run(verify + ["--hex", response, "--allow-signatures"])
        responses.append(response)
    merged_reports = {}
    for name, indices in [("12", [0, 1]), ("13", [0, 2]), ("23", [1, 2]),
                          ("123", [0, 1, 2]), ("duplicate", [0, 1, 0])]:
        output = work / f"merged-{name}.hex"
        command = cast + ["merge-exit", "--manifest", manifest, "--out-hex", output] + custody_args
        for i in indices:
            command += ["--hex", responses[i]]
        run(command)
        merged_reports[name] = json.loads(run(verify + ["--hex", output, "--allow-signatures",
                                                       "--require-fully-signed"]))
    expected_id = json.loads(manifest.read_text())["protocol"]["kaspa_tx_id"]
    assert all(r["kaspa_tx_id"] == expected_id and r["fully_signed"] for r in merged_reports.values())
    report = {
        "kaspad_commit": run(["git", "rev-parse", "HEAD"], cwd=args.kaspad_source).strip(),
        "wallet_version": run([args.wallet, "version"]).strip(),
        "cast_version": run([args.cast, "--version"]).strip(),
        "go_sign_command_succeeded": True,
        "go_returned_pst_accepted": False,
        "go_preserved_lane": go_inspection["lane_id"] == unsigned_inspection["lane_id"],
        "go_preserved_tx_id": go_inspection["kaspa_tx_id"] == unsigned_inspection["kaspa_tx_id"],
        "prepared_compute_budgets": [i["compute_budget"] for i in unsigned_inspection["inputs"]],
        "go_returned_compute_budgets": [i["compute_budget"] for i in go_inspection["inputs"]],
        "go_verification_error": (go_check.stderr + go_check.stdout).strip(),
        "independent_cast_signers": 3,
        "merged_quorums": merged_reports,
        "kaspa_tx_id": expected_id,
        "configured_exit_address": address["address"],
        "test_script_override_used": False,
        "wrong_custody_pin_rejected": True,
        "broadcast": False,
        "fixture": "Disposable keys; nonexistent input; local script validation only",
    }
    (work / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
