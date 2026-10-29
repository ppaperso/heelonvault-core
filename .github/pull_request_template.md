<!--
Contribution guide: CONTRIBUTING.md (EN) / CONTRIBUTING.fr.md (FR).
PR title = squash commit message on main: `<type>(<scope>): <summary>`.
-->

## Summary

<!-- What changes and why, in a few lines. -->

## Linked issue

Closes #

## Validation

<!-- Commands you ran and their result. Without heelonvault-premium:
     scripts/check-core-standalone.sh --semver -->

```text

```

## Checklist

- [ ] The change stays within the contribution scope (`heelonvault-core` / documentation for
      external contributors, see CONTRIBUTING).
- [ ] Quality gates pass: `fmt`, `clippy`, `clippy-secure`, `test`, `semver-checks`.
- [ ] Tests are added or updated for behaviour changes.
- [ ] No public API break of `heelonvault-core`, or the break was agreed in the issue.
- [ ] Dependencies unchanged, or `sbom.cyclonedx.json` regenerated (`./scripts/generate-sbom.sh`)
      and the new dependency agreed in the issue.
- [ ] User-visible changes: changelog (FR and EN) and relevant guides updated.
- [ ] New user-visible strings: keys added to both `locales/fr` and `locales/en`.
- [ ] No secret, credential or personal data in the code, tests, logs or fixtures.

## Security

<!-- Required if the change touches cryptography, authentication, key or secret handling,
     storage, clipboard, logging, network access or dependencies: what changes, the risk, and why
     it is acceptable. Otherwise write "Not security-sensitive". -->
