// Pairing API — RustDesk-style short-code agent claim flow.

export async function lookupPairCode(code) {
    return this.request('/pairing/lookup', {
        method: 'POST',
        body: { code }
    });
}

export async function claimPairedAgent({ pair_code, passphrase, name, group_id, trust_fingerprint }) {
    const body = { code: pair_code, passphrase };
    if (name) body.name = name;
    if (group_id) body.group_id = group_id;
    if (trust_fingerprint) body.trust_fingerprint = trust_fingerprint;
    return this.request('/pairing/claim', {
        method: 'POST',
        body
    });
}
