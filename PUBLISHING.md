# GitHub publication checklist

This repository is prepared locally. **No remote, public repository, push, tag or release is implied by these files.** Obtain the owner's explicit approval for the destination, visibility and public contents before publishing.

1. Select and confirm the actual GitHub owner and repository name. There are intentionally no fabricated repository URLs or badges in the README.
2. Review the exact files to publish. Exclude dist/, target/, executables, local reports, dumps, credentials and private device identifiers. Keep source, Cargo.lock, documentation, license texts and workflows.
3. Review source and run formatting, tests, Clippy, the real tray smoke test and controlled manual UI checks.
4. Review the generated portable archive and SHA256SUMS.txt. Binaries are unsigned previews, not certified builds.
5. With approval, create/configure the GitHub repository (a private staging repository is the safer first step), connect origin, and push the reviewed development branch. Use the owner's configured identity; do not invent an author email or publish private history.
6. Run Windows CI in GitHub and inspect the actual result before claiming it passed. Headless CI intentionally does not validate Bluetooth sound.
7. After code/version validation, create the reviewed version tag. The optional `Prepare draft release` workflow takes an existing tag, validates syntax and package version, runs checks, and prepares a **draft prerelease**. It never publishes a release on its own.
8. Inspect the draft, assets, checksums, privacy, license texts and release notes. Only publish/change repository visibility after the owner's final approval.

Recommended initial visibility is private, initial release status is prerelease, and release notes must preserve the known connected-but-silent and delayed-resume limitations. Do not promise universal stability or iPhone support without evidence.

The prepared workflow definitions have not been validated by a remote GitHub run merely because the local checks pass. The release workflow requires the repository's GitHub Actions token to have the scoped contents permission declared in that job; no personal token is stored in this repository.
