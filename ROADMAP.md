## Ingestion module [COMPLETED]

- [x] Lightroom-style ingestion wizard modal (`Ctrl+I`, File Menu, Top Bar, and Navigator sidebar).
- [x] Automatic detection of mounted SD/CF cards and system drives, with manual folder browse option.
- [x] "Small View" thumbnail preview grid of detected clips with on-the-fly frame 0 thumbnail generation, checkboxes, file sizes, and metadata badges.
- [x] Full support for spanned companion files (`.M00`, `.M01`, etc.) including 5D Mark III multi-card spans (CF + SD): all chunks belonging to each clip are identified and copied together into the specified destination folder.
- [x] User-specified destination folder with persistent memory in config, plus organization options (direct, date-based `YYYY-MM-DD`, or custom subfolder).
- [x] Buffered streaming copy engine with real-time transfer speed (MB/s), file and overall progress bars, and cancellation support.
- [x] Multi-tier integrity verification: post-copy byte size check, MLVI container magic header check, and full IEEE 802.3 CRC-32 checksum hashing verification.
- [x] Automatic database catalog registration of copied clips upon completion.

## Navigation [COMPLETED]

- [x] Mouse side "Back" button click (`Extra1`) support: seamlessly navigates back from Player view (windowed or borderless fullscreen) to Contact Sheet Grid view, or closes open modals.
- [x] Mouse side "Forward" button click (`Extra2`): navigates forward into Cinema Player view for the selected clip.
- [x] Universal keyboard shortcuts: `Alt + Left` (Back) and `Alt + Right` (Forward) mirroring browser and OS standards, alongside `Esc` and `G`.
- [x] Tooltip and keyboard shortcuts modal updates (`F1`).

## Better filters UX

Bug: For the filters, if I click a camera, remove the lenses tags that are no longer available for the clips in the filter, and viceversa. So only possible convinations are listed. Plus in the filter row to show "clear all filters" button, and the raiting filter be a dropdown, rather than buttons, and remove the sort from the filter row. And make sure the row for filters it's inside the box of the clkips, right now it's outside, shoudl so Search should be in nexty to library rather than up`.

## Integration with MLVApp [COMPLETED]
 
- [x] Single-session batch launching: MLV App CLI only takes one file argument in standard mode, so mlvExplorer generates an on-the-fly MLV App Session XML file (`.masxml`) and passes it to MLV App. All selected clips open together in a single MLV App window and playlist.
- [x] Multi-clip selection support: Right-click card context menu opens all selected clips in one MLV App session (`Open in MLV App (N clips)`).
- [x] Inspector integration: Metadata Inspector's "Open in MLV App" button dynamically updates with selection count and opens all selected clips together.
- [x] Navigator sidebar integration: Right-click any Keyword, Smart Collection ("All Clips", "Picks", "Rejected", "Unflagged") to open all associated clips in MLV App.
- [x] Menu & keyboard shortcuts: Studio Top Bar File menu offers `Open Selected in MLV App` (`Ctrl+M`) and `Open Catalog in MLV App...`.
- [x] Automatic chunk deduplication: Multi-chunk spanned clips (`.M00`, `.M01`, etc.) are cleanly deduplicated to primary `.MLV` files so MLV App does not produce duplicate clip references.
- [x] Robust XML escaping: Session generator safely escapes special characters (`&`, `<`, `>`, `"`, `'`) and normalizes path separators for cross-platform Qt compatibility.

## UI Improvments

- Add loaders where the operation takes too long.
- Improve keyboard handling (eg. add keywords from the sidebar not just the manager, make sure they aree added with enter and not only the mouse)
- Add tooltips.
- Make the ui more responsive and "smooth".
- Make more different the grid view from the player view. Hide fields that are not relevant for the current mode (eg the library sidebar in the player view).

## Known bugs

The autodetection for anamorphic mode fails. Maybe we need to add a special check with heruristics for the 16 9 ratios. And add a rotate button for vertical videos (where the 16 9 is vertical).

## Refactor queue widget.

Improve the queue widget to show logs, have multiple progress bar (eg importing and coping files), show errors, etc.
