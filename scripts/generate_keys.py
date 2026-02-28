#!/usr/bin/env python3
"""
Rove SECURE Key Generator
Generates Ed25519 keypairs entirely in memory.
Saves the Private Keys directly to macOS Keychain.
Asks for requirements first, then outputs keys in a table.
"""

import subprocess
import sys

def run_cmd(cmd, input_data=None):
    process = subprocess.run(
        cmd,
        input=input_data,
        capture_output=True,
        text=True
    )
    if process.returncode != 0:
        print(f"Error running command: {' '.join(cmd)}")
        print(process.stderr)
        sys.exit(1)
    return process.stdout

def generate_ed25519():
    priv_key = run_cmd(["openssl", "genpkey", "-algorithm", "ed25519"])
    pub_key = run_cmd(["openssl", "pkey", "-pubout"], input_data=priv_key)
    return priv_key.strip(), pub_key.strip()

def save_to_keychain(service: str, account: str, secret: str):
    # -U updates the item if it already exists
    cmd = ["security", "add-generic-password", "-s", service, "-a", account, "-w", secret, "-U"]
    run_cmd(cmd)

def main():
    print("======================================================")
    print("  Rove Interactive Vault Key Generator")
    print("======================================================")
    print("Keys never touch your disk. They go straight to macOS")
    print("Keychain and print here for you to copy into Infisical.\n")

    # Environment selection
    env_choice = ""
    while env_choice not in ['1', '2']:
        print("Select Environment:")
        print("  1) Development (dev)")
        print("  2) Production (prod)")
        env_choice = input("Enter 1 or 2: ").strip()

    env = "dev" if env_choice == '1' else "prod"
    print(f"\n[Environment selected: {env.upper()}]\n")

    available_keys = [
        {"desc": "Core Tool Key", "for": "core"},
        {"desc": "Official Plugin Key", "for": "plugin"},
        {"desc": "Community Plugin Key", "for": "community"}
    ]

    # Ask for requirements first
    selected_keys = []
    print("Which keys do you need to generate?")
    for k in available_keys:
        ans = input(f"Generate '{k['desc']}' for {env}? (y/N): ").strip().lower()
        if ans == 'y':
            selected_keys.append(k)

    if not selected_keys:
        print("\nNo keys selected. Exiting...")
        return

    print("\nGenerating keys in memory and saving to Keychain...")

    results = []
    for k in selected_keys:
        priv, pub = generate_ed25519()
        k_for = k["for"]

        # Save to keychain
        service_name = f"rove-{k_for}-key-{env}"
        save_to_keychain(service_name, "rove-engine", priv)

        # Formatted names
        priv_name = f"{env}_private_{k_for}_key".upper()
        pub_name = f"{env}_public_{k_for}_key".upper()

        results.append((priv_name, priv))
        results.append((pub_name, pub))

    # Print results in .env format
    print("\n" + "=" * 80)
    print("  .env FORMAT OUTPUT (Ready to import into Infisical/Vault)")
    print("=" * 80 + "\n")

    for name, value in results:
        # Strip the BEGIN and END lines entirely
        clean_lines = [line for line in value.split('\n') if not line.startswith("-----")]
        # Join into a single continuous base64 string
        clean_value = "".join(clean_lines).strip()
        print(f'{name}="{clean_value}"')

    print("\n" + "=" * 80)
    print("\n[SUCCESS] Private keys stashed securely in macOS Keychain.")
    print("Copy the .env format text above and use the 'Import' button in your Vault.\n")

if __name__ == "__main__":
    # Ensure macOS environment
    if sys.platform != "darwin":
        print("Error: This script is designed for macOS Keychain only.")
        sys.exit(1)

    # Catch KeyboardInterrupt explicitly to avoid stack traces
    try:
        main()
    except KeyboardInterrupt:
        print("\n\nExiting...")
        sys.exit(0)
