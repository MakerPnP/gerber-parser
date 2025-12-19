# Changelog

This project follows semantic versioning.

Possible log types:

- `[added]` for new features.
- `[changed]` for changes in existing functionality.
- `[deprecated]` for once-stable features removed in upcoming releases.
- `[removed]` for deprecated features removed in this release.
- `[fixed]` for any bug fixes.
- `[security]` to invite users to upgrade in case of vulnerabilities.

### v0.4.0 (2025-12-19)

- [added] Support for all gerber commands supported by gerber-types 0.7.0.
- [changed] Stricter error handling for TD commands.

### v0.3.1 (2025-07-25)

- [fixed] Fixed panic when parsing aperture definitions with insufficient arguments.

### v0.3.0 (2025-07-11)

- [added] Proper support for legacy/deprecated gerber command: `IN`.
- [added] Support for G04 comment attributes.
  This is a breaking change, since comments are now wrapped in `CommentContent`.

### v0.2.1 (2025-07-10)

- [added] Support for legacy/deprecated gerber commands: `IP`, `MI`, `SF`, `OF`, `IR`, and `AS`.

### v0.2.0 (2025-06-30)

- [changed] API updates.
- [changed] Updated support for attributes.
- [added] Support for all macros.
- [added] Support for all gerber commands supported by gerber-types 0.4.0.

Note: due to the amount of changes, this changelog is brief, future changelogs should be more detailed.

### v0.1.2 (2022-06-29) and older

Unknown (previous maintainer @NemoAndrea had no changelog file)
