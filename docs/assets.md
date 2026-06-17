# Assets

kiln handles two asset sources:

- the configured main stylesheet, usually `styles.css`
- static files under the configured `public` directory

It does not bundle JavaScript or compile CSS. Build those files outside kiln and place the final files in `public/` or reference the main stylesheet path.

## Main Stylesheet

Config:

```toml
[paths]
styles = "styles.css"
```

Build output:

```text
dist/assets/styles.<hash>.css
```

Templates should use:

```html
<link rel="stylesheet" href="{{ site.stylesheet_href }}">
```

## Public Assets

Files in `public/` are copied to the output directory.

```text
public/
  images/avatar.svg
  app.js
  downloads/resume.pdf
```

Templates can reference them through `asset_url` with a leading `/` to produce root-relative URLs:

```html
<img src="/{{ asset_url(path='images/avatar.svg') }}" alt="Avatar">
<script src="/{{ asset_url(path='app.js') }}"></script>
```

`asset_url` returns a path relative to `public/` (e.g. `images/avatar.<hash>.svg`). Prefix with `/` so links resolve correctly from any page depth.

## Fingerprinting

Fingerprintable assets receive content-hash filenames:

```text
images/avatar.svg -> images/avatar.<hash>.svg
app.js -> app.<hash>.js
```

The mapping is written to:

```text
dist/asset_manifest.json
```

The manifest is stable-sorted to reduce noisy diffs.

## CSS URL Rewriting

CSS files under `public/` are scanned for `url(...)` references to sibling assets.

```css
.logo {
  background-image: url("../images/avatar.svg");
}
```

After build, the CSS points at the fingerprinted image path. Query strings and fragments are preserved when present.

The configured main `styles.css` is emitted as a single hashed stylesheet but is not part of `asset_manifest.json`.

## Missing Assets

If a template references an asset that is not in `public/`, kiln keeps the original path. Run `kiln doctor` when asset references look wrong; it checks project paths and common public asset issues.

## Cleanup

Full `kiln build` recreates the output directory, so stale fingerprint files are removed.

In `serve`, public asset rebuilds use `asset_manifest.json` and known fingerprint naming to remove stale generated files without touching unrelated source files.
