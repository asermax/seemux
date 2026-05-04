# CHANGELOG

<!-- version list -->

## v1.1.0 (2026-05-04)

### Bug Fixes

- **ci**: Use --force instead of --force-with-lease for amend push
  ([`004d964`](https://github.com/asermax/seemux/commit/004d9648341a66c5278a7bd2b56d21c31b8261a9))

### Chores

- **deps**: Add minreq HTTP client dependency
  ([`874d212`](https://github.com/asermax/seemux/commit/874d212769f72ded3fd981dccfb92643ff801431))

- **planning**: Add DLT-004 implementation plan
  ([`eb770e3`](https://github.com/asermax/seemux/commit/eb770e3fc188a74899977fad8e690425e11f77b5))

- **planning**: Drop DLT-002 dependency and raise priority for DLT-004
  ([`1df3550`](https://github.com/asermax/seemux/commit/1df35501f7e70ab56ee2f9583a62c8adf3a53c42))

- **planning**: Mark DLT-004 design as complete
  ([`31f51ef`](https://github.com/asermax/seemux/commit/31f51efd7410a7295276cbd2c412d9d6af1bb69f))

- **planning**: Mark DLT-004 spec as complete
  ([`e01ea93`](https://github.com/asermax/seemux/commit/e01ea93d69aec906115a96cba7a481dd12d7ced9))

- **planning**: Update DLT-004 status to design phase
  ([`d2aa492`](https://github.com/asermax/seemux/commit/d2aa492c218b429cfaf46210e61991bc75a9fd7e))

- **planning**: Update DLT-004 status to plan phase
  ([`7c3f5ad`](https://github.com/asermax/seemux/commit/7c3f5ad79187737c329b8edd661407101a3d80ad))

- **planning**: Update DLT-004 status to spec phase
  ([`0a968f5`](https://github.com/asermax/seemux/commit/0a968f52157654509e14f54a4047b6b10faca7bd))

### Continuous Integration

- **release**: Use build_command for lockfile sync instead of amend hack
  ([`e3e3e80`](https://github.com/asermax/seemux/commit/e3e3e80dffe587bade8602a7fedd1cbb94e0b5fe))

### Documentation

- Reconcile DLT-004 browser tabs into feature documentation
  ([`f643039`](https://github.com/asermax/seemux/commit/f6430398adc65a0df97f32e547c6a32af4f2e3cf))

- **designs**: Add DLT-004 browser tabs design draft
  ([`a2d5d22`](https://github.com/asermax/seemux/commit/a2d5d2293fa61fe7ee49baf1ed15beac52fc506e))

- **designs**: Complete DLT-004 browser tabs design
  ([`68d9580`](https://github.com/asermax/seemux/commit/68d9580204df280fecee81c177d41114ac220ba8))

- **designs**: Revise DLT-004 to per-pane browser state model
  ([`8fd4b9a`](https://github.com/asermax/seemux/commit/8fd4b9a6ad8bff4c78d588372779c62f18705bb7))

- **planning**: Mark DLT-004 batch 1 complete and update status
  ([`1560dba`](https://github.com/asermax/seemux/commit/1560dba64c445309915d5d43371edeac484173f0))

- **planning**: Mark DLT-004 batch 2 complete
  ([`a6b20cb`](https://github.com/asermax/seemux/commit/a6b20cba2e34a8569f75919e00be500ec25d5d2b))

- **planning**: Mark DLT-004 batch 3 complete
  ([`f971485`](https://github.com/asermax/seemux/commit/f971485ad76937de3c8a1d954ea9b617f287ddfa))

- **planning**: Mark DLT-004 batch 4 complete
  ([`431b989`](https://github.com/asermax/seemux/commit/431b989cc789f000e3e6da50c33831cfc9a528d6))

- **planning**: Mark DLT-004 batch 5 complete
  ([`331ca7a`](https://github.com/asermax/seemux/commit/331ca7a40dba2c24a7fc173887ca91754972f3ba))

- **planning**: Mark DLT-004 implementation complete
  ([`53ecc48`](https://github.com/asermax/seemux/commit/53ecc48b5566982387ae1195da3d418160a92129))

- **specs**: Add DLT-004 open web pages in browser tabs spec
  ([`2e1d66e`](https://github.com/asermax/seemux/commit/2e1d66e19ccfb6bde762dc9309878a8d9bc8baf6))

- **specs**: Refine DLT-004 browser tabs acceptance criteria
  ([`103cc36`](https://github.com/asermax/seemux/commit/103cc36e79e479dfb71b2619ad6db828b9a0914e))

### Features

- **app**: Add browser tab UI entry points
  ([`a273eff`](https://github.com/asermax/seemux/commit/a273eff0d496b29e3bc27a855e99bd775186e552))

- **config**: Persist browser pane state in session serialization
  ([`a2d42c6`](https://github.com/asermax/seemux/commit/a2d42c68ac65a460cab2774731cb095e5dbddd05))

- **session**: Add browser session creation and URL polling
  ([`28edba4`](https://github.com/asermax/seemux/commit/28edba410c814152f6b9f19aee036fbd05b85a96))

- **session**: Add SessionType enum and browser session constructor
  ([`8d778e2`](https://github.com/asermax/seemux/commit/8d778e23f6940ce752ce68b0a0ac350d6add090c))

- **session**: Detect browser pane crashes and show error overlay
  ([`66443e8`](https://github.com/asermax/seemux/commit/66443e815905895642259c4f7dffb214b2fbc3b3))

- **session**: Persist and restore browser panes across restarts
  ([`c474aa5`](https://github.com/asermax/seemux/commit/c474aa517f6a615fd843b149369f15b8d6b2bbf4))

- **sidebar**: Add browser pane display with globe icon and URL
  ([`5e19bd2`](https://github.com/asermax/seemux/commit/5e19bd2373d0514746812b243a768a7224f4d1a0))

### Refactoring

- **app**: Extract browser error wiring into shared helpers
  ([`866b117`](https://github.com/asermax/seemux/commit/866b117b91514d71fdc0f8222e4ddd19ec55bdfb))

- **dialogs**: Extract overlay card creation into shared helper
  ([`0865d21`](https://github.com/asermax/seemux/commit/0865d21a0a43d77cdc93393560e8a5761e5dee2a))

- **session**: Cache carbonyl availability check and simplify terminal lookup
  ([`a65cd95`](https://github.com/asermax/seemux/commit/a65cd95d9d3ab054a82abf49ba9767d6caddb97a))

- **session**: Flatten nested if-let in CDP poll loop
  ([`de93d37`](https://github.com/asermax/seemux/commit/de93d37bda765e494a08ede75a626a9c8da9a333))

- **session**: Move CDP polling to background thread
  ([`3ae6cd6`](https://github.com/asermax/seemux/commit/3ae6cd69223a753948bc95c7fa411fe55899deb9))


## v1.0.4 (2026-04-27)

### Bug Fixes

- **ci**: Use --force instead of --force-with-lease for amend push
  ([`004d964`](https://github.com/asermax/seemux/commit/004d9648341a66c5278a7bd2b56d21c31b8261a9))


## v1.0.3 (2026-04-27)

### Bug Fixes

- Regenerate Cargo.lock from clean state
  ([`07c8a9f`](https://github.com/asermax/seemux/commit/07c8a9fcc59f19af00fa2e131c71f1e86af762e4))

- **ci**: Configure git identity for Cargo.lock amend step
  ([`8a6dc04`](https://github.com/asermax/seemux/commit/8a6dc0425e44f23861fc16303096cfba7074be00))


## v1.0.2 (2026-04-27)

### Bug Fixes

- **ci**: Sync Cargo.lock after semantic-release version bump
  ([`962cc5d`](https://github.com/asermax/seemux/commit/962cc5d6b0dea4439aa81fa4ef2300ab21eb17c0))

### Chores

- Sync Cargo.lock to v1.0.1
  ([`5fa4aa4`](https://github.com/asermax/seemux/commit/5fa4aa4162a7fc8ac46c112d21dc6a439715d075))

### Continuous Integration

- **release**: Upgrade actions to latest versions
  ([`b64204c`](https://github.com/asermax/seemux/commit/b64204c83610cac1bd990d7bf9d405712d1dab3c))


## v1.0.1 (2026-04-27)

### Bug Fixes

- **ci**: Remove --locked from quality gates
  ([`55ef978`](https://github.com/asermax/seemux/commit/55ef978865920a95800d63fd5b5abde510feacaf))

- **ci**: Remove --locked from release build step
  ([`9a244aa`](https://github.com/asermax/seemux/commit/9a244aaff464f1fdad6a69834419fe1e26947029))


## v1.0.0 (2026-04-27)

### Bug Fixes

- **ci**: Add clippy component to rust toolchain
  ([`00f1557`](https://github.com/asermax/seemux/commit/00f1557c4088304d9419049cce43133be1d4dff1))

### Build System

- Add .cargo/config.toml for Linux linker configuration
  ([`10f090a`](https://github.com/asermax/seemux/commit/10f090a9d419f05c258b97f35fd22259ebe92169))

### Continuous Integration

- **release**: Switch to semantic-release with conventional commits
  ([`41a7e5b`](https://github.com/asermax/seemux/commit/41a7e5be001e3857c6fbe641c19939c73c3c3d30))

### Documentation

- **deltas**: Add DLT-013 for multi-line URL detection
  ([`63d6a9b`](https://github.com/asermax/seemux/commit/63d6a9b94b4017a1bb830798aedc39385ffb5546))

- **deltas**: Mark DLT-012 as planned
  ([`24b8bec`](https://github.com/asermax/seemux/commit/24b8bec30cfc9638427e41649701fa6eb51c9af7))

### Features

- **terminal**: Add PRIMARY selection copy and middle-click paste
  ([`1d2e500`](https://github.com/asermax/seemux/commit/1d2e500a3e00d910202c9ce7e749421cc6b28a0a))

- **terminal**: Detect URLs wrapped across multiple lines
  ([`61262af`](https://github.com/asermax/seemux/commit/61262af4a6cae1a64eb8ef08c4bdba876209761a))

### Refactoring

- **terminal**: Drop redundant i64 cast on column_count
  ([`5239506`](https://github.com/asermax/seemux/commit/52395062e1e48db6f05c46bd820e4b3096bded07))

- **terminal**: Simplify multi-line URL extraction
  ([`1b49e65`](https://github.com/asermax/seemux/commit/1b49e651f80d6a47e43dd217c7533e2f33b90496))


## v0.34.7 (2026-04-19)

### Chores

- **seemux**: Bump version to 0.34.7
  ([`789dc43`](https://github.com/asermax/seemux/commit/789dc43ec4ee41b9ac105810014931d01d8a09f0))


## v0.34.6 (2026-04-19)

### Bug Fixes

- **dropdown**: Prevent VTE SEGV by keeping terminals mapped during hide animation
  ([`268c777`](https://github.com/asermax/seemux/commit/268c7773b40970c8beec5204a4ec883bda21a0e7))

### Chores

- **seemux**: Bump version to 0.34.6
  ([`b242cbe`](https://github.com/asermax/seemux/commit/b242cbe0cb25313097755cfae914f9d45a9c77ab))

### Documentation

- **deltas**: Add DLT-011 for notification badge clearing on tab activation
  ([`2c7fa03`](https://github.com/asermax/seemux/commit/2c7fa037567f7a4c38e91300df6cc3ed2b41ddcd))

- **deltas**: Add DLT-012 for middle-click primary selection copy and paste
  ([`6cd3e17`](https://github.com/asermax/seemux/commit/6cd3e173e6d7ce21729abd7660cd967af4af98ce))


## v0.34.5 (2026-04-08)

### Bug Fixes

- **hooks**: Account for dropdown visibility in notification suppression and badge clearing
  ([`b8fdb45`](https://github.com/asermax/seemux/commit/b8fdb45996f847a15c27ee754aba7f6b8cd5d623))

### Documentation

- **deltas**: Add DLT-010 for stale notification badge fix
  ([`33ca1b6`](https://github.com/asermax/seemux/commit/33ca1b62f9747e065c7988a4b32f4cc73762d0d4))


## v0.34.4 (2026-04-05)

### Bug Fixes

- **ci**: Sync Cargo.toml version with Cargo.lock
  ([`525e0ee`](https://github.com/asermax/seemux/commit/525e0eeba3f109715e9def1b6dc4f34d140f2dc0))

- **keyboard**: Properly drop RefCell borrow before group expand/collapse callback chain
  ([`7135d63`](https://github.com/asermax/seemux/commit/7135d63d5b51e79239e58b3da3c6e4beb8d35b19))


## v0.34.3 (2026-04-05)

### Bug Fixes

- **sidebar**: Fix RefCell panic when expanding/collapsing groups
  ([`5810799`](https://github.com/asermax/seemux/commit/58107995f0f20dcab873b8247701f04d2cb0ad8a))


## v0.34.2 (2026-04-04)

### Features

- **hooks**: Detect git commands in hook events for branch re-detection
  ([`d122375`](https://github.com/asermax/seemux/commit/d122375cea1fd68b3ce8845e00adf9d7ef4174fa))


## v0.34.1 (2026-04-04)

### Bug Fixes

- **keyboard**: Fix segfault when expanding groups via Ctrl+Shift+.
  ([`6bd1578`](https://github.com/asermax/seemux/commit/6bd157834cbb8e57a16f5ef741b0c1d3c2a804d7))


## v0.34.0 (2026-04-04)

### Documentation

- **planning**: Add DLT-007 — configure Claude binary name for resume and state detection
  ([`1524f0a`](https://github.com/asermax/seemux/commit/1524f0a73e23d381f370611aacf92b95385f4cdf))

- **planning**: Add DLT-008 — sync PR status across tabs in the same repository
  ([`69861a7`](https://github.com/asermax/seemux/commit/69861a7c24a50e530866b874cf01123a6a01bead))

- **planning**: Add DLT-009 — filter hook events from background Claude instances
  ([`9c30bb9`](https://github.com/asermax/seemux/commit/9c30bb9e96e419a0fd45e3a710fb6bdc90ebf75d))

### Features

- **session**: Configure Claude binary aliases for resume and state detection
  ([`088b088`](https://github.com/asermax/seemux/commit/088b088216edfbcfaf1e06ad3e55c3c076df0b94))


## v0.33.7 (2026-03-30)

### Bug Fixes

- **terminal**: Disable scroll-on-output
  ([`e369100`](https://github.com/asermax/seemux/commit/e369100a097345fd4a1d46fc9ad962d996019beb))


## v0.33.6 (2026-03-30)

### Refactoring

- **terminal**: Remove scroll guard logic
  ([`6e3679e`](https://github.com/asermax/seemux/commit/6e3679e889b5464555874a65bf5b65bfba1bdf90))


## v0.33.5 (2026-03-30)

### Bug Fixes

- **terminal**: Restore scroll-on-output for background terminal stickiness
  ([`d87e5e7`](https://github.com/asermax/seemux/commit/d87e5e73c4ccb80164add0724a51a93dcbf84061))

### Documentation

- **planning**: Add DLT-005 and DLT-006 to delta inventory
  ([`cdd7314`](https://github.com/asermax/seemux/commit/cdd7314f452ef2057dda52e1c777a6735733c8db))

- **planning**: Update DLT-006 — rename, revise description, escalate to Critical
  ([`9a6b50e`](https://github.com/asermax/seemux/commit/9a6b50e353ef456da4d9c4bda2c5bd9f3080d200))


## v0.33.4 (2026-03-30)


## v0.33.3 (2026-03-30)

### Bug Fixes

- **session**: Clear notifications when session is destroyed
  ([`182c06b`](https://github.com/asermax/seemux/commit/182c06bfbc67a548ae7b688d8f92989d39211aa3))


## v0.33.2 (2026-03-29)

### Bug Fixes

- **keyboard**: Use translate_key for Ctrl+Shift+. detection
  ([`58b03fa`](https://github.com/asermax/seemux/commit/58b03fa5b339107569d189f34924debe889dfd86))


## v0.33.1 (2026-03-28)

### Features

- **keyboard**: Replace group collapse/expand with Ctrl+Shift+. toggle
  ([`eca5b9f`](https://github.com/asermax/seemux/commit/eca5b9fc05a93cbe24c5b9ab0ee0721250b0be9f))

### Refactoring

- **dropdown**: Replace Option<bool> with ToplevelKind enum
  ([`1a43217`](https://github.com/asermax/seemux/commit/1a432174ce6ef9c3edb087116f46be4fff0ffd9f))


## v0.32.0 (2026-03-28)

### Features

- **dropdown**: Only enter dialog mode for KDE dialog windows, not all apps
  ([`642a13f`](https://github.com/asermax/seemux/commit/642a13f03d5ee9b8f28db820821eed33d54daa60))


## v0.31.0 (2026-03-27)

### Documentation

- **planning**: Add delta inventory with layout and browser tab deltas
  ([`96632f0`](https://github.com/asermax/seemux/commit/96632f074a71dc250015edcdb42a7aa06d33f9ed))

### Features

- **sidebar**: Show pointer cursor on tab rows and underline PR on Ctrl+hover
  ([`466ce5e`](https://github.com/asermax/seemux/commit/466ce5ef897bd0a384abafaba7eef0702823140b))


## v0.30.0 (2026-03-25)

### Features

- **session**: Detect running commands immediately for tab navigation
  ([`b883560`](https://github.com/asermax/seemux/commit/b88356018cbf221313c91a6e4ee8abed20b851b1))


## v0.29.1 (2026-03-25)

### Bug Fixes

- **sidebar**: Only open PR link on Ctrl+click
  ([`98d4927`](https://github.com/asermax/seemux/commit/98d4927e471a667158f08abd5a67b218ea67d948))


## v0.29.0 (2026-03-25)

### Documentation

- Retrofit project documentation with katachi framework
  ([`5c87d22`](https://github.com/asermax/seemux/commit/5c87d2274f605b40cf5c27aeaf1b177a330aefe6))

### Features

- **sidebar**: Auto-scroll to active tab on switch
  ([`6c5bb06`](https://github.com/asermax/seemux/commit/6c5bb06300a537a9f8a5056b2ae6684b94fa07e2))


## v0.28.0 (2026-03-25)

### Features

- **actions**: Copy URL to clipboard on right-click
  ([`91639ac`](https://github.com/asermax/seemux/commit/91639acbd5f7ebfc0280be5a82ffeb96e1accd2e))


## v0.27.8 (2026-03-25)

### Code Style

- **sidebar**: Ellipsize paths from the left
  ([`61d4f67`](https://github.com/asermax/seemux/commit/61d4f67b7d70faa028f53f015549825aeb2da91f))


## v0.27.7 (2026-03-25)

### Bug Fixes

- **session**: Use folder name as default tab title
  ([`236c998`](https://github.com/asermax/seemux/commit/236c9987a499a9e479d8c5967912f0cdaa1527d2))


## v0.27.6 (2026-03-23)

### Bug Fixes

- **keyboard**: Resolve Ctrl+Shift+[/] not triggering on non-US layouts
  ([`29f57c1`](https://github.com/asermax/seemux/commit/29f57c1cd4a86db66aaf43d19a220bbfc15ac10b))

### Documentation

- **readme**: Update with recent features and corrected shortcuts
  ([`7170c09`](https://github.com/asermax/seemux/commit/7170c093b794d68a89a3708ec18ca83d603872c8))


## v0.27.5 (2026-03-23)

### Bug Fixes

- **scroll-guard**: Prevent content desync during TUI re-renders
  ([`261629c`](https://github.com/asermax/seemux/commit/261629ca58ecc2dd67f9d4188639f55b2269ba74))

### Documentation

- Add commit scope convention to CLAUDE.md
  ([`e84475c`](https://github.com/asermax/seemux/commit/e84475cbef11820cb7e2a7f09d039431a7c90fd4))


## v0.27.4 (2026-03-23)

### Bug Fixes

- **seemux**: Delay focus recovery to avoid racing toplevel monitor
  ([`88351d1`](https://github.com/asermax/seemux/commit/88351d1c27142f6990d3b270f91860d02fb5a529))

- **seemux**: Filter empty terminal titles from title-change handler
  ([`f0c3310`](https://github.com/asermax/seemux/commit/f0c33101d8aac3582ad890bceaa716c8cac98295))

- **seemux**: Switch tabs by visible index for keyboard shortcuts
  ([`6f3a4be`](https://github.com/asermax/seemux/commit/6f3a4bece96b9dd0d75a17fd91b83a87886c5179))

- **seemux**: Use delayed resume when expanding session groups
  ([`31b5d15`](https://github.com/asermax/seemux/commit/31b5d15e52cd519d1298df3884bec244221ca09c))


## v0.27.3 (2026-03-23)

### Bug Fixes

- **seemux**: Remove extra left padding from sidebar
  ([`638959d`](https://github.com/asermax/seemux/commit/638959deeefd5738d7edda24a96aab38c42a8569))

- **seemux**: Separate pending resume state from active session tracking
  ([`ca0df23`](https://github.com/asermax/seemux/commit/ca0df230c61ae469bc66e1eecc5d11df012478b3))

### Refactoring

- **seemux**: Remove verbose debug logging from toplevel monitor
  ([`0ad05dd`](https://github.com/asermax/seemux/commit/0ad05ddd1e3537ef29a34f310343598337208f0d))


## v0.27.2 (2026-03-23)

### Bug Fixes

- **seemux**: Add scroll guard to preserve position during TUI re-renders
  ([`b5cd3a1`](https://github.com/asermax/seemux/commit/b5cd3a12093b2c6eaece6d8a11979bba9056d964))


## v0.27.1 (2026-03-23)

### Bug Fixes

- **seemux**: Ignore KDE notification toplevels in dialog detection
  ([`2274fad`](https://github.com/asermax/seemux/commit/2274fad015832044f4d4404b3996f3618db6d7c1))


## v0.27.0 (2026-03-23)

### Features

- **seemux**: Detect external dialogs via ext-foreign-toplevel-list-v1
  ([`681cb9f`](https://github.com/asermax/seemux/commit/681cb9ff049815e6cb1cd14741dc8b338c26f2fb))


## v0.26.2 (2026-03-23)

### Bug Fixes

- **seemux**: Remove unsupported max-height CSS property
  ([`4c7eb75`](https://github.com/asermax/seemux/commit/4c7eb75e00d425b6c056710e74f8c6e8a9fe80c0))


## v0.26.1 (2026-03-22)

### Bug Fixes

- **seemux**: Prevent duplicate claude --resume injection on tab switch
  ([`de8680d`](https://github.com/asermax/seemux/commit/de8680dcdac71eab66d3a0e263a0e269fc543c8a))


## v0.26.0 (2026-03-22)

### Features

- **seemux**: Add rename group option to right-click context menu
  ([`9a60a26`](https://github.com/asermax/seemux/commit/9a60a269eeeae25fadb36e44f765686d8841d955))


## v0.25.3 (2026-03-22)

### Bug Fixes

- **seemux**: Provide multi-size tray icon pixmaps for badge visibility
  ([`3bdb060`](https://github.com/asermax/seemux/commit/3bdb060a8e4db72cbd3d1951233c2f97de2f1067))


## v0.25.2 (2026-03-22)

### Bug Fixes

- **seemux**: Populate session_cwds before register_session
  ([`e233c84`](https://github.com/asermax/seemux/commit/e233c845e955b14eca5c500e61d6743267144063))


## v0.25.1 (2026-03-22)

### Chores

- **seemux**: Remove scroll guard from VTE terminal
  ([`122405f`](https://github.com/asermax/seemux/commit/122405feed484ffef6794ceb92f193082dd82b4b))


## v0.25.0 (2026-03-21)

### Features

- **seemux**: Defer terminal spawning for collapsed groups
  ([`a85079b`](https://github.com/asermax/seemux/commit/a85079b8a92b37d72e2dcbf8f247b3349a240c3a))


## v0.24.3 (2026-03-21)

### Bug Fixes

- **seemux**: Composite tray badge into main icon pixmap
  ([`7e94c43`](https://github.com/asermax/seemux/commit/7e94c432d0b78b1b75c252d7afd880b1074b919a))


## v0.24.2 (2026-03-21)

### Code Style

- **seemux**: Increase system tray notification badge size
  ([`954c428`](https://github.com/asermax/seemux/commit/954c428c4c3f41895a299ae6f1b7ce4ebaa1bb2b))


## v0.24.1 (2026-03-21)

### Chores

- **seemux**: Regenerate logo size variants from updated source
  ([`b0d2a07`](https://github.com/asermax/seemux/commit/b0d2a07305f61d64b001eb616f8096695e783fd1))


## v0.24.0 (2026-03-21)

### Features

- **seemux**: Show folder icon in tab title when displaying current directory
  ([`0cae858`](https://github.com/asermax/seemux/commit/0cae858c318e0db96c819f4b5421e46c73167707))


## v0.23.3 (2026-03-21)

### Bug Fixes

- **seemux**: Harden Running badge suppression for Claude Code sessions
  ([`b8e8dc0`](https://github.com/asermax/seemux/commit/b8e8dc04091f9cdaf8df146b932972ae2b755ed0))


## v0.23.2 (2026-03-21)

### Refactoring

- **seemux**: Decouple VTE4 from non-terminal modules
  ([`9db7320`](https://github.com/asermax/seemux/commit/9db7320de579f0b0eba0330ba31b6c1bbfeecef0))


## v0.23.1 (2026-03-21)

### Bug Fixes

- **seemux**: Prevent false Running badge when launching Claude Code
  ([`d1f0538`](https://github.com/asermax/seemux/commit/d1f0538123a83a262a2f84304d255e99bd84be35))


## v0.23.0 (2026-03-21)

### Bug Fixes

- **seemux**: Prevent stale peek state when switching tabs
  ([`060db36`](https://github.com/asermax/seemux/commit/060db3613fcf790aee1a94d0e80b27dc87656471))

### Features

- **seemux**: Delay Running badge for non-Claude sessions
  ([`b92799b`](https://github.com/asermax/seemux/commit/b92799b6146ed1eaef92fb57c26ce7bf14700f1f))


## v0.22.0 (2026-03-21)

### Features

- **seemux**: Peek running tabs in collapsed groups
  ([`cb77f7d`](https://github.com/asermax/seemux/commit/cb77f7dea9c4cda9a88a6ecffc91824da81b6134))


## v0.21.1 (2026-03-21)

### Bug Fixes

- **seemux**: Remove no-op multiplication in tray badge spacing
  ([`db3512c`](https://github.com/asermax/seemux/commit/db3512c3434e8541f35247a5160383b0364e28cb))

### Features

- **seemux**: Add Ctrl+Shift+PageUp/Down to cycle running tabs
  ([`c191668`](https://github.com/asermax/seemux/commit/c1916681264dca0e9e9470415edcfc6c73a340f9))


## v0.20.3 (2026-03-21)

### Bug Fixes

- **seemux**: Prevent scroll guard frozen_value corruption during VTE cursor jumps
  ([`2eb7f52`](https://github.com/asermax/seemux/commit/2eb7f52dc26530f81eb172fc526ee002cecc7b83))


## v0.20.2 (2026-03-21)

### Bug Fixes

- **seemux**: Clear tab-index overlays on window focus loss
  ([`a991b6d`](https://github.com/asermax/seemux/commit/a991b6d5d970a7d10edd368b5733881f602156f4))


## v0.20.1 (2026-03-21)

### Bug Fixes

- **seemux**: Increase system tray notification badge size
  ([`4dd5cbd`](https://github.com/asermax/seemux/commit/4dd5cbd536c348aba845259ae31d9a1da01a5d60))


## v0.20.0 (2026-03-21)

### Features

- **seemux**: Add Ctrl+Shift+[/] shortcuts to collapse/expand groups
  ([`07974d9`](https://github.com/asermax/seemux/commit/07974d9fc422a24e736b8e755cf0ed3c280dc170))


## v0.19.0 (2026-03-21)

### Features

- **seemux**: Rework system tray with branded icon, click actions, and notification badge
  ([`b52f2ad`](https://github.com/asermax/seemux/commit/b52f2ade4b6108bae96b4f47dcae0aabd2c5331b))


## v0.18.2 (2026-03-21)

### Bug Fixes

- **seemux**: Fix sidebar toggle shortcut not reaching handler
  ([`3a82ead`](https://github.com/asermax/seemux/commit/3a82eade1db98f10e53db73cb11ec7b803ed0899))


## v0.18.1 (2026-03-20)

### Bug Fixes

- **seemux**: Improve tab focus selection order after close
  ([`95f010e`](https://github.com/asermax/seemux/commit/95f010e7ac5eaf3ca4f950bde68e5f4de5c93040))


## v0.18.0 (2026-03-20)

### Features

- **seemux**: Show Running status pill for terminal commands and notify on completion
  ([`7855d5e`](https://github.com/asermax/seemux/commit/7855d5e70b0b5094140d5bf442adc86567384a43))


## v0.17.2 (2026-03-20)

### Bug Fixes

- **seemux**: Add libdbus-1-dev to CI Docker image
  ([`33fd80c`](https://github.com/asermax/seemux/commit/33fd80c688411bbe7c29f4b49c6c479b46caf565))


## v0.17.1 (2026-03-20)

### Bug Fixes

- **seemux**: Fix scroll guard to properly handle VTE viewport jumps during re-renders
  ([`1898354`](https://github.com/asermax/seemux/commit/189835495db12fa222468badba0de666fc3ceeed))


## v0.17.0 (2026-03-20)

### Features

- **seemux**: Add system tray icon with notification badge
  ([`30f0c4b`](https://github.com/asermax/seemux/commit/30f0c4b2dcc4196f3385de1d0e58a8ea98def004))


## v0.16.4 (2026-03-20)

### Bug Fixes

- **seemux**: Remove broken --dangerously-skip-permissions injection in tmux shim
  ([`ef626f5`](https://github.com/asermax/seemux/commit/ef626f575d61474a26d6aa1a45962236bc5c6440))


## v0.16.3 (2026-03-20)

### Bug Fixes

- **seemux**: Remove Ctrl+T keybinding for new tab
  ([`c3f59cc`](https://github.com/asermax/seemux/commit/c3f59ccd1e320733ff210c2710d425ec4fa838c2))

### Documentation

- **marketplace**: Update README with recent features
  ([`163ca21`](https://github.com/asermax/seemux/commit/163ca21c1348cea149bebffa3f240437f67516e6))


## v0.16.2 (2026-03-20)

### Bug Fixes

- **seemux**: Change sidebar toggle keybinding to Ctrl+Shift+B
  ([`68a144f`](https://github.com/asermax/seemux/commit/68a144fc2e1660abd19a9f898b623811714d4cac))


## v0.16.1 (2026-03-20)

### Bug Fixes

- **seemux**: Restore correct active tab on app reopen
  ([`27400d2`](https://github.com/asermax/seemux/commit/27400d251676010f2edfbd35bffd38278aef3f69))

### Features

- **seemux**: Improve collapsed sidebar sizing and add default-run
  ([`565caed`](https://github.com/asermax/seemux/commit/565caedb01e5ef95bbd81220881f031a780ead0a))


## v0.16.0 (2026-03-20)

### Features

- **seemux**: Overhaul sidebar collapse with minimal collapsed view
  ([`3574079`](https://github.com/asermax/seemux/commit/357407925d182bcb95f9937b679ac3730d40115d))


## v0.15.0 (2026-03-19)

### Features

- **seemux**: Add scroll guard to prevent VTE viewport jumps during CLI re-renders
  ([`d91bf51`](https://github.com/asermax/seemux/commit/d91bf5144a30717d9997e138b3bb5cd5ebf9c722))


## v0.14.0 (2026-03-19)

### Features

- **seemux**: Add collapsible sidebar with status dots
  ([`e9f17f6`](https://github.com/asermax/seemux/commit/e9f17f643e52dfaff142296233877d42ffef1d97))


## v0.13.6 (2026-03-18)

### Bug Fixes

- **seemux**: Improve PR detection to handle compound commands and filter open PRs only
  ([`3d2bd0c`](https://github.com/asermax/seemux/commit/3d2bd0cb6041d8880893b7d53ab09e3bc24a9090))


## v0.13.5 (2026-03-18)

### Bug Fixes

- **seemux**: Replace time-based debounce with state-based flag for post-stop notification
  suppression
  ([`37cb138`](https://github.com/asermax/seemux/commit/37cb138857a2603ba0b936fe69a1f9dee1d517e0))


## v0.13.4 (2026-03-18)

### Bug Fixes

- **seemux**: Fix stale SourceId panic in git branch re-detection debounce
  ([`7233717`](https://github.com/asermax/seemux/commit/72337171ec5ffef85143edc20168b6d9e4d8a41e))


## v0.13.3 (2026-03-18)

### Bug Fixes

- **seemux**: Fix RefCell borrow panics causing SIGABRT crashes
  ([`af0594c`](https://github.com/asermax/seemux/commit/af0594c9a149162881e4f8c92e380762681c5f01))


## v0.13.2 (2026-03-18)

### Bug Fixes

- **seemux**: Compact branch row layout and add branch tooltip
  ([`dc32d63`](https://github.com/asermax/seemux/commit/dc32d636656b38e5b4d4f5298e6d22300dcc210e))


## v0.13.1 (2026-03-18)

### Bug Fixes

- **seemux**: Clear notifications on pre-tool-use
  ([`5a5192b`](https://github.com/asermax/seemux/commit/5a5192b25575e5cdbd28250eb42f1fb690d1aa17))


## v0.13.0 (2026-03-18)

### Features

- **seemux**: Detect PR created by Claude via post-tool-use hook
  ([`71ec8f3`](https://github.com/asermax/seemux/commit/71ec8f38d7932d80327fc12087c0535a46862260))

- **seemux-hooks**: Add PostToolUse hook
  ([`675a367`](https://github.com/asermax/seemux/commit/675a3678170de2c58242cc635e243fc7d0d024d6))


## v0.12.0 (2026-03-18)

### Features

- **seemux**: Add Ctrl+Click URL opening in terminal
  ([`fd6a509`](https://github.com/asermax/seemux/commit/fd6a509879b65c2ec90243f470cb6ae0ef46656b))


## v0.11.1 (2026-03-18)

### Bug Fixes

- **seemux**: Fix post-stop notification suppression and clean up notification types
  ([`5637f36`](https://github.com/asermax/seemux/commit/5637f36090ac3a170d7f2d8cc5bb78b63be96635))


## v0.11.0 (2026-03-18)

### Features

- **seemux**: Add robust session state persistence
  ([`7aabbdb`](https://github.com/asermax/seemux/commit/7aabbdb2dfc1609120442b6e52bdae89ce8794b1))


## v0.10.0 (2026-03-18)

### Features

- **seemux**: Handle StopFailure hook event
  ([`7d6e734`](https://github.com/asermax/seemux/commit/7d6e7340f41c9e4c1e77d6a5005f278fe98d484e))

- **seemux-hooks**: Add StopFailure hook and make all hooks async
  ([`2ac3b71`](https://github.com/asermax/seemux/commit/2ac3b719b110bd70eb6b4fda7e2e40dea62c85fe))


## v0.9.0 (2026-03-17)

### Features

- **seemux**: Auto-inject --allow-dangerously-skip-permissions for subagents
  ([`09a1a69`](https://github.com/asermax/seemux/commit/09a1a6982be3b1841e1caeed2f5db81c92209a56))


## v0.8.0 (2026-03-17)

### Features

- **seemux**: Add per-terminal tmux-mode toggle via shim subcommand
  ([`35f299e`](https://github.com/asermax/seemux/commit/35f299e56cdee950775a529825140a94ca544bd5))


## v0.7.11 (2026-03-17)

### Bug Fixes

- **seemux**: Keep tabs with unread notifications peeking in collapsed groups
  ([`c431b75`](https://github.com/asermax/seemux/commit/c431b7599ac5edc42635a1d65c9acc2256c89592))


## v0.7.10 (2026-03-17)

### Bug Fixes

- **seemux**: Suppress notification events arriving shortly after stop
  ([`0a28328`](https://github.com/asermax/seemux/commit/0a28328b3dd1cfb7423f693442091f8a6ca71725))


## v0.7.9 (2026-03-17)

### Bug Fixes

- **seemux**: Decode percent-encoded characters in tab subtitle paths
  ([`908d741`](https://github.com/asermax/seemux/commit/908d74196a4f71a2cb25b024f58d9769ef72a07d))


## v0.7.8 (2026-03-17)

### Bug Fixes

- **seemux**: Include tmux shim in CI release and remove TMUX env var
  ([`fded043`](https://github.com/asermax/seemux/commit/fded04349666cb36f161abb5f42040c15b90a52f))


## v0.7.7 (2026-03-17)

### Bug Fixes

- **seemux**: Restore TMUX env variable in agent shim
  ([`b130f30`](https://github.com/asermax/seemux/commit/b130f307bc5365151919563fc251407987b35e19))


## v0.7.6 (2026-03-17)

### Bug Fixes

- **seemux**: Use Capture phase for keypress tracking controller
  ([`582cf97`](https://github.com/asermax/seemux/commit/582cf97ac5b215fd1343358fb38a247ed6bdb478))


## v0.7.5 (2026-03-17)

### Bug Fixes

- **seemux**: Remove TMUX and COLORTERM env vars from agent shim
  ([`b4f5d51`](https://github.com/asermax/seemux/commit/b4f5d512eabf689d8bba5ca817ae26a3ee81ba22))


## v0.7.4 (2026-03-17)

### Bug Fixes

- **seemux**: Move main Claude session into team group on teammate creation
  ([`579050f`](https://github.com/asermax/seemux/commit/579050f791d8117968e83272d65c96ff59ee9b7e))


## v0.7.3 (2026-03-17)

### Bug Fixes

- **seemux**: Set COLORTERM=truecolor for tmux shim sessions
  ([`5c674f3`](https://github.com/asermax/seemux/commit/5c674f3a0addf015b5790a8cb500c53b83b663b7))


## v0.7.2 (2026-03-17)

### Bug Fixes

- **seemux**: Peek active tab when collapsing its group
  ([`4aacdea`](https://github.com/asermax/seemux/commit/4aacdea834e0ce86a3dae6e523d47f4af8d7a161))


## v0.7.1 (2026-03-17)

### Bug Fixes

- **seemux**: Pre-type claude resume command for collapsed group sessions
  ([`1369d67`](https://github.com/asermax/seemux/commit/1369d6771cd4990a003a362c2d5008c461152aa0))


## v0.7.0 (2026-03-17)

### Features

- **seemux**: Add Agent Teams support via tmux shim
  ([`3975ed6`](https://github.com/asermax/seemux/commit/3975ed6fcc6a06de32a4ab040da476536173f795))


## v0.6.1 (2026-03-17)

### Bug Fixes

- **seemux**: Fix blocking recv in async git helpers and quality improvements
  ([`82b7975`](https://github.com/asermax/seemux/commit/82b79752310f49a5db4d7b6cf78f436605c9dbe0))

### Chores

- **seemux-hooks**: Remove PostToolUse hook
  ([`1d645d4`](https://github.com/asermax/seemux/commit/1d645d493c5ddf8a2831a8dec7a7dedf8610d36f))


## v0.6.0 (2026-03-17)

### Documentation

- **seemux**: Update README and CLAUDE.md for app module split
  ([`8c260b6`](https://github.com/asermax/seemux/commit/8c260b63d706ed338e6e0f161f3f0996f87efc11))

### Features

- **seemux**: Add PR number display on tab rows
  ([`2179373`](https://github.com/asermax/seemux/commit/21793738c847c7c3b1e5d40e0ecd1944ac01af6a))


## v0.5.1 (2026-03-16)

### Chores

- **seemux**: Bump version to 0.5.1
  ([`588a3bc`](https://github.com/asermax/seemux/commit/588a3bc05f7e93f6e0478beac778da0e64d8ebce))

- **seemux**: Fix clippy warnings and suppress intentional ones
  ([`abd95dd`](https://github.com/asermax/seemux/commit/abd95dd0a44496a7144ca4b1b12fd3fdbf19f842))

### Refactoring

- **seemux**: Split app.rs into sub-modules
  ([`ae31c28`](https://github.com/asermax/seemux/commit/ae31c28536e48b6fa6317fae298bb532a2cc905f))


## v0.5.0 (2026-03-16)

### Features

- **seemux**: Add peek functionality for collapsed groups
  ([`3c30e62`](https://github.com/asermax/seemux/commit/3c30e627d95e02aeea887b0a41f51c6b4a37277d))


## v0.4.1 (2026-03-16)

### Bug Fixes

- **seemux**: Allow Ctrl+Tab to work regardless of Shift state
  ([`7c4af08`](https://github.com/asermax/seemux/commit/7c4af0870c9b1dfb5f888fbbce1d5e39004e4ef7))

### Chores

- **seemux**: Bump version to 0.4.1
  ([`4da5e93`](https://github.com/asermax/seemux/commit/4da5e93335adf0f62f277fa9d770cf1d8da9e797))


## v0.4.0 (2026-03-16)

### Chores

- **marketplace**: Add clippy to CLAUDE.md dev commands
  ([`e995274`](https://github.com/asermax/seemux/commit/e9952746ee4de9eac8e3516c4b47c0b53359de64))

- **marketplace**: Update GitHub Actions to latest versions
  ([`4f07dd1`](https://github.com/asermax/seemux/commit/4f07dd12761403b73bc954904af1283ce9b5b3d8))

### Features

- **seemux**: Persist and restore group collapsed state
  ([`38b1bff`](https://github.com/asermax/seemux/commit/38b1bffeace4b6272716e970c1554551ae66ad5c))


## v0.3.0 (2026-03-16)

- Initial Release
