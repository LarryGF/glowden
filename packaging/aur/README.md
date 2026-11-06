# AUR packages

- `glowden`: builds the tagged release from source.
- `glowden-bin`: repackages the Arch package attached to the GitHub release.

Both were built and installed in a clean `archlinux:base-devel` container for 0.3.0.

## Publishing (needs an AUR account with an SSH key)

```sh
git clone ssh://aur@aur.archlinux.org/glowden.git aur-glowden
cp packaging/aur/glowden/{PKGBUILD,.SRCINFO,glowden.install} aur-glowden/
cd aur-glowden && git add -A && git commit -m "glowden 0.3.0" && git push
```

Same for `glowden-bin`.

## Each release

Set `pkgver`, reset `pkgrel=1`, update `sha256sums` (`updpkgsums`), regenerate
`.SRCINFO` (`makepkg --printsrcinfo > .SRCINFO`) and push both AUR repos. From 0.4.0 the
`glowden` PKGBUILD should also install the GNOME focus extension, as
`packaging/arch/PKGBUILD` does.
