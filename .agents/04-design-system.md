# VibeInspector Design System

## Design read

VibeInspector is a professional developer workbench for senior engineers and technical leads. It should feel closer to a precise native inspector or database tool than a consumer dashboard.

Design dials: `DESIGN_VARIANCE=4`, `MOTION_INTENSITY=3`, `VISUAL_DENSITY=8`.

## 1. Visual direction

- Zinc is the dominant palette in light and dark modes.
- Blue is a narrow signal for selection, focused evidence, and links; it is not a page theme.
- Green, amber, and red appear only for semantic status.
- Use one restrained Zinc gradient across the full application frame. Project and module rails stay transparent over that shared plane; crisp separators, selected rows, and framed tools provide hierarchy without turning each rail into a colour block.
- Crisp borders and spacing define structure. Rounded containers must retain a precise 1px edge so the application feels engineered rather than soft or inflated.
- Use a four-level depth scale: compact controls, repeated raised items, independent tool panels, and dialogs. Every shadow includes a restrained inner highlight and a Zinc-tinted outer shadow; elevation must still communicate real hierarchy rather than decorate every row.
- Keep a consistent soft-corner system: 12px for framed tools, 10-15px for repeated raised items and dialogs, and 6-10px for compact controls. Interior panes may stay square when they are clipped by a rounded parent. Pill shapes remain reserved for status indicators.
- A low-contrast Zinc-on-Zinc gradient spans the entire window to prevent a flat single-plane background. It may carry only a trace of signal blue, must remain subordinate to content, and must work in both themes.
- No colorful gradient blobs, glass, glow, decorative background shapes, nested cards, oversized headings, or marketing composition.

## 2. Layout

- Baseline window: `1440x900`; minimum: `1100x680`.
- Reserve a `38px` overlay titlebar at the top of the webview. It contains the centered application title and the global theme action; the top-left traffic-light area stays free of app controls.
- Do not repeat the application brand in the project rail or add a second full-width page titlebar below the system titlebar. Project/module context belongs with the selected flow heading.
- Project rail: `216px`, collapsible to `56px`.
- Module rail: `280px`, collapsible to `48px`.
- Inspector: `minmax(0, 1fr)` with independently scrolling content.
- The inspector has no overview/code tabs. It is divided vertically into an upper architecture band and a lower source evidence band.
- The upper band contains one Mermaid diagram surface and one un-nested rationale panel.
- The lower band contains one `220-240px` file rail and a read-only Monaco surface; the minimum-width desktop breakpoint may reduce the rail to `200px`.
- Keep stable heights and widths so selection, loading, labels, and status do not shift the shell.
- First launch uses a focused setup workspace, not a marketing landing page. Codex history is a privacy-scoped discovery row, followed by two directory command rows and an explicit candidate review step.

## 3. Zinc tokens

`src/app.css` is the token source of truth. Use semantic variables rather than direct zinc hex values in reusable components:

```css
--background;
--workspace;
--panel;
--titlebar;
--foreground;
--card;
--muted;
--muted-foreground;
--border;
--input;
--ring;
--sidebar;
--sidebar-accent;
--signal;
--status-success;
--status-warning;
--status-error;
--status-info;
--app-backdrop;
--workspace-surface;
--project-rail-surface;
--module-rail-surface;
--titlebar-surface;
--panel-border;
--control-shadow;
--control-shadow-active;
--inset-shadow;
--item-shadow;
--selected-shadow;
--panel-shadow;
--dialog-shadow;
```

The dark default uses adjacent Zinc 900-950 values rather than pure or near-pure black: one shared gradient carries the window, the transparent rails are separated by crisp rules, and framed tools are the lightest surfaces. Light mode uses a Zinc 50/100 gradient with white tools. `--signal` is the only non-semantic blue. Application gradients interpolate only between adjacent Zinc surfaces and may mix no more than a trace of `--signal`.

## 4. Typography

- UI font: macOS system stack (`-apple-system`, `SF Pro Text`, then `Helvetica Neue`). This is the default because the product is macOS-only and benefits from native optical sizing and rasterization.
- Code, paths, hashes, and line ranges: SF Mono / JetBrains Mono / `ui-monospace`.
- Rail text: 11-13px; body and controls: 13-14px; metadata: 10px minimum; page title: 24px maximum.
- Letter spacing is zero except tiny uppercase metadata where scanning materially improves.
- Do not force grayscale font smoothing. Use platform-default smoothing and `font-optical-sizing: auto`.
- Truncate paths and branch names; expose the full value through title or tooltip.

## 5. Component ownership

| Surface                                               | Component source                                                         |
| ----------------------------------------------------- | ------------------------------------------------------------------------ |
| Buttons, badges, input, checkbox, separator, skeleton | shadcn-svelte local components                                           |
| Dialogs, scroll areas, tooltips, dropdowns            | shadcn-svelte wrappers backed by Bits UI                                 |
| Icons                                                 | `@lucide/svelte`                                                         |
| Project/module rows                                   | Domain components composed from shadcn tokens                            |
| Flow nodes/edges                                      | Mermaid through `src/lib/adapters/diagram.ts` and a local Svelte adapter |
| Code                                                  | Local Monaco adapter, always read-only                                   |

Do not recreate a shadcn control inside a feature directory. Do not put VibeInspector domain assumptions into `src/lib/components/ui`.

## 6. Interaction rules

- Every icon-only button has an accessible label and tooltip/title.
- Keyboard focus uses `--ring`; focus must remain visible on Zinc surfaces.
- Selected rows use a restrained muted fill plus border; never a saturated full-row color.
- Project status dots align to the same right-hand action axis as the project rail add button.
- Hover changes background or foreground only; no large translate/scale movement.
- Preserve the selected project, module, flow, and file across refreshes.
- Respect `prefers-reduced-motion`; ordinary transitions stay within 120-180ms.
- Rail expansion uses a 260ms grid-track transition with a short opacity/translate content reveal. Reduced motion removes both transitions.
- Inline errors remain next to the affected scope; toast is only for transient confirmation.

## 7. Diagram and editor

- Mermaid is the only flow renderer. Domain entities keep renderer-neutral nodes and edges; the adapter generates safe Mermaid syntax with generated node aliases and escaped labels.
- Mermaid uses `securityLevel: 'strict'`, loads dynamically in the browser, and re-renders when the application theme changes.
- Diagram surfaces, nodes, borders, labels, and edges use the Zinc diagram tokens in `src/app.css`. Blue is not used as a general edge color.
- The diagram has compact zoom controls, no ornamental legend, and may scroll horizontally when a narrow window cannot fit the graph.
- The rationale is readable at `45-60ch` and explains architectural tradeoffs, not implementation narration.
- Rationale body text is at least `12px` in the desktop baseline. Confidence and metadata may be smaller, but never determine the reading size.
- Monaco uses `readOnly` and `domReadOnly`, disables context editing, hides the minimap, and highlights referenced lines.
- File rows show name, path, and line range with clear selected/stale states.
- The source rail is a keyboard-accessible hierarchical tree. Folders sort before files and repeated ranges for one file are aggregated.

## 8. Secondary surfaces

- Activity, settings, MCP health, and keyboard help use shadcn dialogs backed by Bits UI focus management.
- Settings use a compact segmented theme control, real tray/MCP socket health, and a checkbox for the LaunchAgent login item.
- Service state is never inferred from the desktop window and the browser preview reports services as unavailable rather than fabricating an online state.
- Loading dialogs and the initial shell use shape-preserving shadcn Skeleton components rather than centered spinners.

## 9. Visual QA

Verify `1440x900` and `1100x680`, light/dark, expanded/collapsed rails, long labels, nonblank Mermaid output, upper/lower band sizing, source switching, empty/error/loading states, and reduced motion. There must be no overlaps, clipped actions, illegible Zinc contrast, unexpected layout shifts, or browser console errors.
