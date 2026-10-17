#!/usr/bin/env python3
"""Envoie les métadonnées App Store de PatchPad (src-tauri/appstore/metadata.json) vers
App Store Connect : catégories, nom, sous-titre, URL de confidentialité, description,
mots-clés, texte promotionnel, URLs, copyright, numéro de version et notes pour la revue.

Usage : scripts/appstore_metadata.py [--dry-run]

Prérequis : la fiche de l'app existe dans App Store Connect (elle ne peut pas être créée par
l'API) et la clé API est lue dans l'environnement ou dans `.env` à la racine du dépôt
(APPLE_API_KEY, APPLE_API_ISSUER, APPLE_API_KEY_PATH). Seule la bibliothèque standard et
`openssl` sont utilisés ; les identifiants ne sont jamais affichés.
"""

import base64
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

APP_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = APP_DIR.parent
METADATA = APP_DIR / "src-tauri" / "appstore" / "metadata.json"
TAURI_CONF = APP_DIR / "src-tauri" / "tauri.conf.json"
API = "https://api.appstoreconnect.apple.com"
# Fiches publiées : leurs métadonnées ne sont plus modifiables.
LOCKED_STATES = {"READY_FOR_DISTRIBUTION", "READY_FOR_SALE", "REPLACED_WITH_NEW_INFO"}

DRY_RUN = "--dry-run" in sys.argv


def fail(message):
    print(f"erreur : {message}", file=sys.stderr)
    sys.exit(1)


def load_env():
    env = dict(os.environ)
    dotenv = REPO_ROOT / ".env"
    if dotenv.exists():
        for line in dotenv.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if line and not line.startswith("#") and "=" in line:
                key, value = line.split("=", 1)
                env.setdefault(key.strip(), value.strip().strip('"').strip("'"))
    for key in ("APPLE_API_KEY", "APPLE_API_ISSUER", "APPLE_API_KEY_PATH"):
        if not env.get(key):
            fail(f"{key} manquant (environnement ou .env).")
    if not Path(env["APPLE_API_KEY_PATH"]).is_file():
        fail("APPLE_API_KEY_PATH ne pointe pas vers la clé .p8.")
    return env


def b64url(data):
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def der_to_raw(der):
    """Signature ECDSA DER → r||s sur 64 octets (format JWT ES256)."""
    i = 2 if der[1] < 0x80 else 2 + (der[1] & 0x7F)
    parts = []
    for _ in range(2):
        length = der[i + 1]
        parts.append(der[i + 2:i + 2 + length].lstrip(b"\x00").rjust(32, b"\x00"))
        i += 2 + length
    return b"".join(parts)


class Client:
    def __init__(self, env):
        self.env = env

    def token(self):
        header = {"alg": "ES256", "kid": self.env["APPLE_API_KEY"], "typ": "JWT"}
        now = int(time.time())
        payload = {"iss": self.env["APPLE_API_ISSUER"], "iat": now, "exp": now + 900, "aud": "appstoreconnect-v1"}
        signing_input = f"{b64url(json.dumps(header).encode())}.{b64url(json.dumps(payload).encode())}"
        der = subprocess.run(
            ["openssl", "dgst", "-sha256", "-sign", self.env["APPLE_API_KEY_PATH"]],
            input=signing_input.encode(), capture_output=True, check=True,
        ).stdout
        return f"{signing_input}.{b64url(der_to_raw(der))}"

    def request(self, method, path, body=None, params=None):
        url = API + path + ("?" + urllib.parse.urlencode(params) if params else "")
        if method != "GET" and DRY_RUN:
            print(f"  [dry-run] {method} {path} {json.dumps(body, ensure_ascii=False)[:160]}")
            return {"data": {"id": "dry-run"}}
        data = json.dumps(body).encode() if body is not None else None
        req = urllib.request.Request(url, data=data, method=method)
        req.add_header("Authorization", f"Bearer {self.token()}")
        if data is not None:
            req.add_header("Content-Type", "application/json")
        try:
            with urllib.request.urlopen(req) as resp:
                raw = resp.read()
                return json.loads(raw) if raw else None
        except urllib.error.HTTPError as err:
            detail = err.read().decode(errors="replace")
            try:
                errors = json.loads(detail).get("errors", [])
                detail = "; ".join(f"{e.get('title')} — {e.get('detail')}" for e in errors)
            except ValueError:
                pass
            raise RuntimeError(f"{method} {path} → {err.code} : {detail}") from None


def upsert(client, items, locale, type_, attributes, parent_rel, parent_type, parent_id):
    """Met à jour la localisation `locale` si elle existe, sinon la crée."""
    existing = next((i for i in items if i["attributes"]["locale"] == locale), None)
    if existing:
        client.request("PATCH", f"/v1/{type_}/{existing['id']}",
                       {"data": {"type": type_, "id": existing["id"], "attributes": attributes}})
        return "mise à jour"
    client.request("POST", f"/v1/{type_}", {"data": {
        "type": type_,
        "attributes": {"locale": locale, **attributes},
        "relationships": {parent_rel: {"data": {"type": parent_type, "id": parent_id}}},
    }})
    return "créée"


def main():
    meta = json.loads(METADATA.read_text(encoding="utf-8"))
    version = json.loads(TAURI_CONF.read_text(encoding="utf-8"))["version"]
    client = Client(load_env())

    apps = client.request("GET", "/v1/apps", params={"filter[bundleId]": meta["bundleId"]})["data"]
    if not apps:
        fail(f"aucune app « {meta['bundleId']} » dans App Store Connect : créez d'abord sa fiche "
             "(Apps › + › Nouvelle app, plateforme macOS).")
    app_id = apps[0]["id"]
    print(f"App {apps[0]['attributes']['name']} ({app_id}){' — simulation' if DRY_RUN else ''}")

    # --- Informations de l'app : catégories, nom, sous-titre, confidentialité.
    infos = client.request("GET", f"/v1/apps/{app_id}/appInfos")["data"]
    info = next((i for i in infos if (i["attributes"].get("state") or i["attributes"].get("appStoreState"))
                 not in LOCKED_STATES), None)
    if info is None:
        fail("aucune fiche d'informations modifiable (version déjà publiée ?).")
    client.request("PATCH", f"/v1/appInfos/{info['id']}", {"data": {
        "type": "appInfos", "id": info["id"],
        "relationships": {
            "primaryCategory": {"data": {"type": "appCategories", "id": meta["primaryCategory"]}},
            "secondaryCategory": {"data": {"type": "appCategories", "id": meta["secondaryCategory"]}},
        },
    }})
    print(f"Catégories : {meta['primaryCategory']}, {meta['secondaryCategory']}")
    localizations = client.request("GET", f"/v1/appInfos/{info['id']}/appInfoLocalizations")["data"]
    for locale, loc in meta["locales"].items():
        result = upsert(client, localizations, locale, "appInfoLocalizations",
                        {"name": loc["name"], "subtitle": loc["subtitle"], "privacyPolicyUrl": loc["privacyPolicyUrl"]},
                        "appInfo", "appInfos", info["id"])
        print(f"Informations {locale} : {result}")

    # --- Version en préparation (créée si besoin) : numéro, copyright, textes.
    versions = client.request("GET", f"/v1/apps/{app_id}/appStoreVersions",
                              params={"filter[platform]": "MAC_OS"})["data"]
    editable = [v for v in versions
                if (v["attributes"].get("appVersionState") or v["attributes"].get("appStoreState")) not in LOCKED_STATES]
    attributes = {"versionString": version, "copyright": meta["copyright"]}
    if editable:
        version_id = editable[0]["id"]
        client.request("PATCH", f"/v1/appStoreVersions/{version_id}",
                       {"data": {"type": "appStoreVersions", "id": version_id, "attributes": attributes}})
    else:
        created = client.request("POST", "/v1/appStoreVersions", {"data": {
            "type": "appStoreVersions",
            "attributes": {"platform": "MAC_OS", **attributes},
            "relationships": {"app": {"data": {"type": "apps", "id": app_id}}},
        }})
        version_id = created["data"]["id"]
    print(f"Version {version} ({version_id})")

    version_locs = (client.request("GET", f"/v1/appStoreVersions/{version_id}/appStoreVersionLocalizations")["data"]
                    if version_id != "dry-run" else [])
    for locale, loc in meta["locales"].items():
        fields = {k: loc[k] for k in ("description", "keywords", "promotionalText", "supportUrl", "marketingUrl")}
        result = upsert(client, version_locs, locale, "appStoreVersionLocalizations", fields,
                        "appStoreVersion", "appStoreVersions", version_id)
        print(f"Textes {locale} : {result}")

    # --- Notes pour l'équipe de revue (les coordonnées se saisissent dans App Store Connect).
    try:
        detail = (client.request("GET", f"/v1/appStoreVersions/{version_id}/appStoreReviewDetail")
                  if version_id != "dry-run" else None)
    except RuntimeError:
        detail = None
    if detail and detail.get("data"):
        detail_id = detail["data"]["id"]
        client.request("PATCH", f"/v1/appStoreReviewDetails/{detail_id}", {"data": {
            "type": "appStoreReviewDetails", "id": detail_id, "attributes": {"notes": meta["reviewNotes"]}}})
    else:
        client.request("POST", "/v1/appStoreReviewDetails", {"data": {
            "type": "appStoreReviewDetails", "attributes": {"notes": meta["reviewNotes"]},
            "relationships": {"appStoreVersion": {"data": {"type": "appStoreVersions", "id": version_id}}},
        }})
    print("Notes pour la revue : OK")
    print("Terminé. Restent à compléter dans App Store Connect : captures d'écran, confidentialité de "
          "l'app (« Aucune donnée collectée »), prix, classification par âge, coordonnées de contact.")


if __name__ == "__main__":
    try:
        main()
    except RuntimeError as error:
        fail(str(error))
