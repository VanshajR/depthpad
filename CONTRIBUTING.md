# Contributing to Depthpad

Thanks for helping. Every kind of contribution is welcome: bug reports, testing with your keyboard, keyboard reports, code and docs.

## The most useful things right now

1. **Try Depthpad with a keyboard marked Untested** in the [README](README.md#keyboards) and tell us what happened, in a [keyboard support request](https://github.com/VanshajR/depthpad/issues/new?template=keyboard-support.yml).
2. **Own a Hall-effect keyboard that isn't listed?** Create a [keyboard report](docs/KEYBOARD_REPORTS.md) and attach it to a keyboard support request.
3. **Add support for a keyboard**: see [docs/ADDING_A_KEYBOARD.md](docs/ADDING_A_KEYBOARD.md).

## Reporting a bug

Use the [bug report form](https://github.com/VanshajR/depthpad/issues/new?template=bug.yml). A session recording (About → Record a session) helps a lot with input problems, but it contains every key you pressed while recording: only record while reproducing the problem, and read it before attaching it.

## Code

- Build and test as described in [docs/BUILDING.md](docs/BUILDING.md). `cargo test --workspace` and `npm run check` must pass.
- Keep the style of the code around your change. Pure logic (protocols, curves, routing) goes in testable modules without I/O, with unit tests.
- Never send a keyboard a command you haven't seen its own software send.
- Keep it light: Depthpad runs while people play games. Check memory and CPU before and after UI changes.
- One topic per pull request, with what you changed and how you tested it.

## Licence

Depthpad is licensed under the GNU AGPL-3.0. By contributing, you agree that your contribution is licensed under it too. Code taken from elsewhere must have a compatible licence and be credited in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Conduct

Be kind and assume good intent. Harassment or personal attacks aren't tolerated; maintainers may remove comments or contributors who don't follow this.
