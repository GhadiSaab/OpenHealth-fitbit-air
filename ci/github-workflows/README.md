These are GitHub Actions workflows. They are kept here because the automated
session that created them could not write to `.github/workflows/`. Move them
there to enable CI and the debug APK build:

    mkdir -p .github/workflows && git mv ci/github-workflows/*.yml .github/workflows/
