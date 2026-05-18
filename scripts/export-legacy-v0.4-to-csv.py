#!/usr/bin/env python3
"""Export legacy v0.4 passwordmanager DB to v1.1 CSV format.

CSV output columns (required by HeelonVault 1.1 import):
name,url,username,password,notes
"""

from __future__ import annotations

import argparse
import base64
import csv
import getpass
import json
import sqlite3
from pathlib import Path
from urllib.parse import urlparse

from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.ciphers.aead import AESGCM
from cryptography.hazmat.primitives.kdf.pbkdf2 import PBKDF2HMAC

PBKDF2_ITERATIONS = 600_000
KEY_LEN = 32


def derive_key(master_password: str, salt: bytes) -> bytes:
    kdf = PBKDF2HMAC(
        algorithm=hashes.SHA256(),
        length=KEY_LEN,
        salt=salt,
        iterations=PBKDF2_ITERATIONS,
    )
    return kdf.derive(master_password.encode("utf-8"))


def decrypt_password(password_data: str, aesgcm: AESGCM) -> str:
    payload = json.loads(password_data)
    nonce_b64 = payload["nonce"]
    ciphertext_b64 = payload["ciphertext"]
    nonce = base64.b64decode(nonce_b64)
    ciphertext = base64.b64decode(ciphertext_b64)
    plaintext = aesgcm.decrypt(nonce, ciphertext, None)
    return plaintext.decode("utf-8")


def normalize_url(url: str) -> tuple[str, str]:
    value = (url or "").strip()
    if not value:
        return "", ""

    parsed = urlparse(value)
    if parsed.scheme in {"http", "https"}:
        return value, ""

    if not parsed.scheme and "." in parsed.path and " " not in parsed.path:
        return f"https://{value}", f"legacy url normalized from '{value}'"

    return "", f"legacy url dropped (invalid for import): '{value}'"


def resolve_sources(args: argparse.Namespace) -> tuple[Path, Path, str]:
    """Resolve db/salt paths from profile, workspace UUID or explicit paths."""
    legacy_dir = Path(args.legacy_dir).expanduser().resolve()

    if args.db_path or args.salt_path:
        if not args.db_path or not args.salt_path:
            raise SystemExit(
                "[ERROR] --db-path and --salt-path must be provided together"
            )
        db_path = Path(args.db_path).expanduser().resolve()
        salt_path = Path(args.salt_path).expanduser().resolve()
        label = f"custom-db:{db_path.name}"
        return db_path, salt_path, label

    if args.workspace_uuid:
        db_path = legacy_dir / f"passwords_{args.workspace_uuid}.db"
        salt_path = legacy_dir / f"salt_{args.workspace_uuid}.bin"
        label = f"workspace:{args.workspace_uuid}"

        users_db = legacy_dir / "users.db"
        if users_db.is_file():
            try:
                conn = sqlite3.connect(str(users_db))
                row = conn.execute(
                    "SELECT username FROM users WHERE workspace_uuid = ?",
                    (args.workspace_uuid,),
                ).fetchone()
                conn.close()
                if row and row[0]:
                    label = f"workspace:{args.workspace_uuid} user:{row[0]}"
            except sqlite3.Error:
                # Keep UUID-based label if users.db cannot be queried.
                pass

        return db_path, salt_path, label

    db_path = legacy_dir / f"passwords_{args.profile}.db"
    salt_path = legacy_dir / f"salt_{args.profile}.bin"
    label = f"profile:{args.profile}"
    return db_path, salt_path, label


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Export legacy passwordmanager DB to HeelonVault 1.1 CSV"
    )
    parser.add_argument(
        "--legacy-dir",
        default=str(Path.home() / ".local/share/passwordmanager"),
        help="Legacy data directory (default: ~/.local/share/passwordmanager)",
    )
    parser.add_argument(
        "--profile",
        help="Legacy profile name used in files passwords_<profile>.db and salt_<profile>.bin",
    )
    parser.add_argument(
        "--workspace-uuid",
        help="Workspace UUID used in files passwords_<uuid>.db and salt_<uuid>.bin",
    )
    parser.add_argument(
        "--db-path",
        help="Explicit path to legacy passwords DB",
    )
    parser.add_argument(
        "--salt-path",
        help="Explicit path to legacy salt file",
    )
    parser.add_argument(
        "--output",
        required=True,
        help="Output CSV path",
    )
    args = parser.parse_args()

    selected_modes = sum(
        [
            1 if args.profile else 0,
            1 if args.workspace_uuid else 0,
            1 if args.db_path or args.salt_path else 0,
        ]
    )
    if selected_modes != 1:
        raise SystemExit(
            "[ERROR] choose exactly one source mode: --profile OR --workspace-uuid OR (--db-path + --salt-path)"
        )

    db_path, salt_path, source_label = resolve_sources(args)
    output_path = Path(args.output).expanduser().resolve()

    if not db_path.is_file():
        raise SystemExit(f"[ERROR] Legacy DB not found: {db_path}")
    if not salt_path.is_file():
        raise SystemExit(f"[ERROR] Legacy salt not found: {salt_path}")

    master_password = getpass.getpass(f"Master password for {source_label}: ")
    if not master_password:
        raise SystemExit("[ERROR] Empty password is not allowed")

    salt = salt_path.read_bytes()
    key = derive_key(master_password, salt)
    aesgcm = AESGCM(key)

    conn = sqlite3.connect(str(db_path))
    conn.row_factory = sqlite3.Row

    rows = conn.execute(
        """
        SELECT title, username, password_data, url, notes
        FROM passwords
        ORDER BY id ASC
        """
    ).fetchall()

    output_path.parent.mkdir(parents=True, exist_ok=True)

    exported = 0
    failed = 0

    with output_path.open("w", newline="", encoding="utf-8") as fh:
        writer = csv.DictWriter(
            fh,
            fieldnames=["name", "url", "username", "password", "notes"],
        )
        writer.writeheader()

        for row in rows:
            title = (row["title"] or "").strip()
            username = (row["username"] or "").strip()
            raw_url = (row["url"] or "").strip()
            notes = (row["notes"] or "").strip()

            try:
                password = decrypt_password(row["password_data"], aesgcm)
            except Exception as exc:  # noqa: BLE001
                failed += 1
                print(f"[WARN] id=? title='{title}': decrypt failed ({exc})")
                continue

            normalized_url, url_note = normalize_url(raw_url)
            merged_notes = notes
            if url_note:
                merged_notes = f"{notes} | {url_note}" if notes else url_note

            writer.writerow(
                {
                    "name": title,
                    "url": normalized_url,
                    "username": username,
                    "password": password,
                    "notes": merged_notes,
                }
            )
            exported += 1

    conn.close()

    print(f"[INFO] Source: {source_label}")
    print(f"[INFO] DB:     {db_path}")
    print(f"[INFO] Salt:   {salt_path}")
    print(f"[OK] CSV exported: {output_path}")
    print(f"[INFO] Exported entries: {exported}")
    print(f"[INFO] Failed entries:   {failed}")
    if failed > 0:
        print("[NEXT] Verify master password/profile and retry if needed.")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
