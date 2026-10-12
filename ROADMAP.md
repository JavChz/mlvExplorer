## Ingestion module

Add the option to ingest .mlv from an SD and CF Card to the catalog and copy the files to the computer (specially important for the 5D III card span files, that requiere to live in the same folders). Let the user chose the folder to copy with a wizard. Make sure the files are copied (and not only referenced). In the case of the 5D III files, make sure that the span files are copied alongside the main file, inside the same folder.

## Better filters UX

Bug: For the filters, if I click a camera, remove the lenses tags that are no longer available for the clips in the filter, and viceversa. So only possible convinations are listed. Plus in the filter row to show "clear all filters" button, and the raiting filter be a dropdown, rather than buttons, and remove the sort from the filter row. And make sure the row for filters it's inside the box of the clkips, right now it's outside, shoudl so Search should be in nexty to library rather than up`.

## Integration with MLVApp

Being able to open a whole catalog, keyword or selection of clips into MLVApp. Right now if I click open in MLVApp it just opens multiple instances of MLVApp instead of one with all the files open in that session.

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
