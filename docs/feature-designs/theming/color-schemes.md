# Design: Color Schemes

## Retrofit Note

Inferred from existing code at: `src/theme.rs`
Retrofit date: 2026-03-24

---

## Problem Context

Seemux is a GTK4-based terminal multiplexer that renders multiple UI surfaces -- terminal emulators, a sidebar with tab rows, status indicators, overlays, drag-and-drop feedback, a dropdown/quake mode, and a system tray. All of these surfaces must share a cohesive visual identity. The challenge is threefold:

1. **Heterogeneous rendering APIs**: VTE terminals use a `set_colors` API (foreground, background, 16-color palette), GTK widgets are styled through CSS providers, and the collapsed sidebar draws directly via Cairo. A single color scheme definition must feed all three pathways.
2. **Multiple window modes**: The same scheme must be applied in both the main window and the dropdown/quake window, which are constructed independently at different points in the application lifecycle.
3. **User configurability**: Users expect to choose between popular terminal color schemes without editing CSS or code.

- **Constraints**: Single-threaded GTK event loop; schemes must be resolvable synchronously at startup from a TOML config file; no external asset files beyond the compiled binary.
- **Interactions**: Config subsystem (TOML parsing), VTE4 terminal widget API, GTK4 CSS provider, Cairo drawing context, system tray (ksni/libappindicator).
- **Scope**: Defines and applies color schemes. Does not handle font configuration, layout sizing, or animation timing (though CSS transitions are embedded in the generated stylesheet).

## Design Overview

The theming system is a pure-data, static-dispatch design. A `ColorScheme` struct holds all color values as `&'static str` hex/rgba strings. Two scheme instances are compiled into the binary as `static` constants. At startup, the configured scheme name is looked up from a registry slice, and the resolved scheme reference fans out to three independent rendering pipelines:

1. **CSS generation** -- A `generate_css` function interpolates scheme colors into a GTK4 CSS template string, which is loaded into a `CssProvider` once at application startup.
2. **VTE terminal coloring** -- Each `VteTerminal` instance parses the scheme's hex strings into `gdk::RGBA` values and calls the VTE `set_colors` API.
3. **Cairo draw colors** -- The `CollapsedBar` precomputes floating-point RGB tuples from the scheme's hex strings for zero-allocation use inside draw callbacks.

A fourth, lighter integration passes the scheme's accent color string directly to the system tray setup function.

## Modeling

### ColorScheme (Central Value Object)

```
ColorScheme
├── name: &'static str             (scheme identifier for lookup)
├── Terminal Colors
│   ├── terminal_fg: &'static str  (foreground hex)
│   ├── terminal_bg: &'static str  (background hex)
│   └── palette: [&'static str; 16]  (16-color ANSI palette)
├── UI Chrome Colors
│   ├── window_bg
│   ├── sidebar_bg
│   ├── accent                     (brand/highlight color)
│   ├── text_primary
│   ├── text_secondary
│   ├── text_muted
│   ├── separator
│   ├── surface_hover              (rgba string)
│   └── surface_active             (rgba string)
└── Status Colors
    ├── status_running
    ├── status_needs_input
    ├── status_error
    ├── status_completed
    └── status_idle
```

### DrawColors (Derived, Precomputed)

```
DrawColors
├── idle: (f64, f64, f64)
├── running: (f64, f64, f64)
├── needs_input: (f64, f64, f64)
├── error: (f64, f64, f64)
├── completed: (f64, f64, f64)
└── accent: (f64, f64, f64)
```

Constructed once from a `ColorScheme` reference via `from_scheme`, stored in the `CollapsedBar` behind `Rc<DrawColors>`.

### Scheme Registry

```
SCHEMES: &[&ColorScheme] = &[&CATPPUCCIN_MOCHA, &DRACULA]
```

A static slice of references. Lookup is a linear scan by name, with Catppuccin Mocha as the fallback default.

## Data Flow

### 1. Entry: Scheme Resolution at Startup

The application's `connect_startup` handler reads `config.color_scheme` (a `String` from TOML deserialization) and calls `theme::get_scheme(&name)`. This performs a linear search over the `SCHEMES` slice and returns a `&'static ColorScheme`. Unknown names silently fall back to `CATPPUCCIN_MOCHA`.

### 2. Process: Three Rendering Pipelines

**Pipeline A -- GTK CSS (applied once globally)**

In `connect_startup`, `theme::generate_css(scheme)` produces a complete CSS stylesheet by interpolating scheme color values into a `format!` template. The result is loaded into a `gtk4::CssProvider` and registered at `STYLE_PROVIDER_PRIORITY_APPLICATION` on the default display. This is a one-shot operation; the CSS is never regenerated at runtime.

**Pipeline B -- VTE Terminal Colors (applied per terminal)**

Each `VteTerminal::new_with_config` independently calls `theme::get_scheme` to obtain the scheme, then `apply_colors` parses the hex strings into `gdk::RGBA` via `gdk::RGBA::parse` and calls `terminal.set_colors`. This happens every time a terminal is created.

**Pipeline C -- Cairo Draw Colors (applied once per sidebar)**

When a `Sidebar` is constructed, it creates a `CollapsedBar::new(scheme)`, which converts the scheme's status and accent hex colors into `(f64, f64, f64)` tuples via a `parse_hex` helper. These are stored in `Rc<DrawColors>` and captured by Cairo draw function closures, avoiding per-frame allocation or parsing.

**Pipeline D -- System Tray Accent (applied once)**

Both `build_window` and `build_quake_window` pass `scheme.accent` (the raw `&str`) to `tray::setup_tray`, which parses it independently for badge rendering.

### 3. Output: Styled UI Across All Surfaces

All GTK widgets pick up their colors from the CSS provider. Terminals display the scheme's palette. Collapsed sidebar dots render using precomputed RGB. The tray badge uses the accent color.

### 4. Errors

Color parsing uses `.expect()` in both the VTE path (`gdk::RGBA::parse`) and the collapsed bar path (`parse_hex` with fallback to `128`). Invalid hex strings in the VTE path will panic. The collapsed bar's `parse_hex` gracefully falls back to a mid-gray `(0.5, 0.5, 0.5)` per component. Scheme resolution never fails due to the `unwrap_or` fallback.

## Key Decisions

### Decision 1: Static Constants Over Runtime Loading

**Choice**: Color schemes are defined as `pub static` constants compiled into the binary, not loaded from external files at runtime.

**Why**: Eliminates file I/O errors, path resolution, and parsing failures at startup. Guarantees that the application always has valid theme data available. Aligns with the project's single-binary distribution model.

**Alternatives Not Chosen**:
- External TOML/JSON theme files (would allow user-created themes, but adds failure modes and file discovery complexity).
- Embedded resource files via GResource (adds build-time complexity for no structural benefit since the data is just string constants).

**Consequences**:
- Positive: Zero runtime overhead for scheme availability; compile-time guarantees on data shape.
- Negative: Adding a new scheme requires recompilation. Users cannot create custom themes without modifying source code.

**ADR/DES Candidate**: Yes -- ADR. This is a one-time architectural choice about how theme data is packaged. Relevant if/when custom user themes are considered.

### Decision 2: Runtime CSS Generation via String Interpolation

**Choice**: The GTK CSS stylesheet is generated at runtime by interpolating scheme values into a `format!()` string template, rather than using precompiled CSS files or a CSS preprocessor.

**Why**: Allows a single CSS template to serve any scheme. No build-time CSS toolchain is needed. The template is co-located with the scheme data in the same module, making the relationship between colors and their usage explicit.

**Alternatives Not Chosen**:
- Multiple precompiled CSS files (one per scheme) -- duplicates layout/sizing rules across files, creating maintenance burden.
- CSS custom properties (`--accent-color`) -- GTK4's CSS engine has limited support for CSS variables compared to web browsers; behavior can be inconsistent.
- GResource-embedded CSS with placeholder substitution -- adds build complexity without meaningful benefit.

**Consequences**:
- Positive: Single source of truth for all CSS rules. Adding a new scheme requires zero CSS work.
- Negative: The CSS template string is large (~400 lines in the format macro) and hard to syntax-check at compile time. CSS errors only surface at runtime via GTK warnings.

**ADR/DES Candidate**: Yes -- ADR. This decision trades compile-time CSS validation for flexibility and simplicity.

### Decision 3: Separate Color Application Paths per Rendering Backend

**Choice**: Three independent subsystems (GTK CSS, VTE API, Cairo drawing) each parse and apply colors from the scheme independently, rather than sharing a single pre-parsed color representation.

**Why**: Each backend requires a different color representation: GTK CSS needs string values, VTE needs `gdk::RGBA`, and Cairo needs `(f64, f64, f64)`. A shared representation would still require conversion at each call site.

**Alternatives Not Chosen**:
- A universal pre-parsed color type (e.g., storing `gdk::RGBA` in the scheme) -- would couple the scheme struct to GTK types, preventing it from being a plain data definition. `gdk::RGBA` also cannot be constructed in a `const`/`static` context.
- A conversion layer that outputs all formats from a single parse pass -- adds complexity for minimal benefit since parsing happens at most once per scheme per lifetime.

**Consequences**:
- Positive: Each rendering path is self-contained and easy to reason about. The scheme struct remains a simple, dependency-free data structure.
- Negative: Hex parsing is duplicated in three places (`gdk::RGBA::parse`, `parse_hex` in collapsed_bar, `parse_hex_color` in tray). Error handling is inconsistent (panic vs. fallback).

**ADR/DES Candidate**: No. This is a natural consequence of the rendering backend diversity rather than a deliberate architectural choice.

### Decision 4: Scheme Lookup via Linear Scan with Silent Fallback

**Choice**: `get_scheme` performs a linear scan of the `SCHEMES` slice and falls back to Catppuccin Mocha if the name is not found. No warning is emitted.

**Why**: With only two schemes in the registry, linear scan is optimal. Silent fallback avoids startup noise for users who may have stale config files.

**Alternatives Not Chosen**:
- `HashMap` lookup -- unnecessary overhead for two entries, and `HashMap` cannot be constructed as a `static`.
- Returning `Option` and letting callers decide -- would push fallback logic to every call site.
- Logging a warning on fallback -- would be more helpful for debugging config issues.

**Consequences**:
- Positive: Simple, infallible API. Callers never need to handle "scheme not found."
- Negative: A typo in `config.toml` silently applies the wrong scheme with no user feedback. Diagnosing "why is my Dracula theme not working" requires knowing about this fallback behavior.

**ADR/DES Candidate**: No. Minor implementation choice, though the silent fallback could be revisited (adding an `eprintln!` warning) if users report confusion.

### Decision 5: Per-Terminal Scheme Resolution

**Choice**: Each `VteTerminal::new_with_config` independently calls `theme::get_scheme` rather than receiving a pre-resolved scheme reference.

**Why**: Keeps `VteTerminal` construction self-contained. The terminal only needs a `Config` reference, which it already receives for font/scrollback settings.

**Alternatives Not Chosen**:
- Passing `&ColorScheme` into `VteTerminal::new_with_config` -- would require callers to resolve the scheme and thread it through.
- Storing the scheme in `AppState` for shared access -- adds coupling for negligible performance benefit.

**Consequences**:
- Positive: `VteTerminal` has no dependency on calling context beyond `Config`. Easy to test or construct in isolation.
- Negative: The scheme is re-resolved on every terminal creation (trivial cost for a 2-element scan).

**ADR/DES Candidate**: No. Standard encapsulation trade-off with negligible impact.

## System Behavior

### Scenario: Startup with Valid Scheme

- **Given**: `config.toml` contains `color_scheme = "dracula"`
- **When**: The application starts
- **Then**: `get_scheme("dracula")` returns the Dracula static reference; CSS is generated with Dracula colors and loaded into the display's CSS provider; the sidebar and collapsed bar use Dracula colors; the system tray receives `"#bd93f9"` as the accent color

### Scenario: Startup with Unknown Scheme

- **Given**: `config.toml` contains `color_scheme = "solarized-dark"`
- **When**: The application starts
- **Then**: `get_scheme("solarized-dark")` silently returns `CATPPUCCIN_MOCHA`; all surfaces render with Catppuccin Mocha colors; no error or warning is emitted

### Scenario: Terminal Creation

- **Given**: The application is running with Dracula scheme
- **When**: A new terminal tab is created
- **Then**: `VteTerminal::new_with_config` resolves the scheme from config, parses 18 hex strings into `gdk::RGBA`, and calls `set_colors` on the VTE widget; the terminal renders with Dracula's foreground, background, and 16-color palette

### Scenario: Collapsed Sidebar Dot Rendering

- **Given**: The sidebar is collapsed and a session has status "error"
- **When**: The session's dot is drawn via Cairo
- **Then**: The draw callback reads the precomputed `DrawColors.error` tuple and calls `cr.set_source_rgb` with those values; no string parsing occurs during the draw call

### Scenario: Dropdown Mode Theming

- **Given**: The application is launched in quake/dropdown mode
- **When**: The dropdown window is built
- **Then**: `theme::get_scheme` is called independently from the dropdown constructor; the same CSS provider (registered globally on the display in `connect_startup`) styles the dropdown's widgets; the `.dropdown-border` class picks up the scheme's accent color

## Notes

- **Inconsistent error handling for color parsing**: The VTE path panics on invalid hex (`expect("valid fg color")`), while `collapsed_bar::parse_hex` gracefully falls back to mid-gray. Since both consume the same `&'static str` constants, this inconsistency is currently harmless but could surface if scheme data were ever loaded from external sources.
- **CSS provider is global, scheme resolution is repeated**: The CSS provider is registered once on the display in `connect_startup`, but scheme resolution via `get_scheme` is called independently in `build_window`, `build_quake_window`, `DropdownWindow::new`, and every `VteTerminal` construction. These all resolve to the same static reference, so there is no inconsistency, but it reveals that the scheme reference is not centralized in `AppState`.
- **Transition timing is hardcoded in CSS**: Animation durations (150ms, 200ms) are embedded in the CSS template, not derived from the scheme or config. If these ever need to be configurable, the CSS template would need additional interpolation parameters.
- **No runtime theme switching**: The CSS provider is loaded once at startup. Changing the scheme requires restarting the application. The `CssProvider` could theoretically be reloaded, but no mechanism exists for this.
- **Surface hover/active use rgba strings, not hex**: The `surface_hover` and `surface_active` fields use `rgba(...)` format strings rather than hex, because they require alpha transparency. This works because GTK CSS accepts rgba syntax, but it means the `ColorScheme` struct mixes two color string formats.
