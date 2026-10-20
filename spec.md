# sprcmtrd - specification

## Overview 

The `sprcmtrd` project is a Rust-based implementation of COMTRADE file viewer.
Terminal command line application with tui (text user interface) based on ratatui.

COMTRADE (Common Format for Transient Data Exchange for Power Systems) is an IEEE standard (IEEE Std C37.111) file format used to store oscillography and status data from power system disturbances, faults, and simulation tests.

- **Binary name:** `clai`
- **Language / runtime:** Rust 2021, async via Tokio

## Architecture

main.rs - entry point of the application.
   lib.rs - library root of the application.
   args.rs - module handling the command line arguments parsing.
   comtrade/
      mod.rs - module handling the COMTRADE file parsing and processing.
	  reader.rs - module handling the reading of COMTRADE files.
	  parser.rs - module handling the parsing of COMTRADE files.
   command/
      mod.rs - module handling the commands part of the application.
	  info.rs - info command handling module.
	  list.rs - list command handling module.
	  view.rs - view command handling module.
   tui/
      mod.rs - module handling the TUI components of the application. Start point for the TUI functionality.
	  app.rs - module handling tui state and key handle events.
	  comtrade.rs - layout for displaying COMTRADE file details.
	  filepicker.rs - overlay layout for COMTRADE file search.
	  theme.rs - module handling the TUI theme and styling.
	  

## Modules 

### Comtrade module

comtrade/mod.rs
comtrade/reader.rs
comtrade/parser.rs

Comtrade object: 
* Configuration data parsed from the COMTRADE configuration file (.cfg).
* Analog channels iterator
* Binary channels iterator
* Methods:
  * from_cfg - method to create a Comtrade object from the configuration file (.cfg).
  * from_files - method to create a Comtrade object from both the configuration file (.cfg) and the sample file (.dat).

Analog object:
  * Analog channel metadata from the COMTRADE configuration file (.cfg).
  * Sample data for the analog channels in generic types int16, int32, or float32.

Binary object:
  * Binary channel metadata from the COMTRADE configuration file (.cfg).
  * Sample data for the binary channels.


## TUI module

Handles the TUI module which provides a text-based user interface for interacting with the application.
Use ratatui library for building the text-based user interface.

to run the TUI use run function in the `tui` module.
* use tokio_select and EventStream for handling asynchronous events in the TUI.
* use ratatui widgets for building the TUI components.


### Theme

tui/theme.rs

* Full configuration for the TUI theme.
* Declarate Dark and light themes for the TUI.
* Use system colors for better integration with the terminal environment.
* Allow users to switch between dark and light themes dynamically.
* Provide a consistent look and feel across different TUI components.


### Layout declaration
 
Header: 
* left Big text sprcmtrd, version
* right Current COMTRADE file information
Main: 
*Analog channels lineplots
*Binary channels status plots
Footer:
* Status bar with key hints and application status

* When filepicker layout is active, it overlays this main layout.


### comtrade layout

* Displays detailed information about the currently selected COMTRADE file, including analog and binary channel data.
* Displays lineplots for the analog channels.
* Displays status plots for the binary channels.
* Can be accessed from the main TUI layout.
* Lineplots can be zoomed and panned for detailed analysis.

### filepicker layout
* Overlay layout showing the search interface for COMTRADE files.
* Open with key combination ctrl+shift+p
* Provides an overlay for searching COMTRADE files.
* Allows users to input search queries and displays matching results.
* Allows selection of a COMTRADE file in current directory.
* Use regular expressions for filtering search results.

## Command module

in mod module is entry point for the command handling part of the application.
and arg enum for command line arguments. Command enum defines the different commands available in the application.
to run some command use async method run on the Command enum.

### Info command

command/info.rs

Info command arguments.
* `file` - The path to the COMTRADE file for which information is to be displayed.

Handles the `info` command which displays detailed information about a COMTRADE file.

### List command

command/list.rs

Handles the `list` command which lists all available COMTRADE files in the specified directory.

* Search for COMTRADE files in current directory.
* Shows the list of COMTRADE files found in the current directory.
* Prints information from .dat file in tabular format.

### View command

command/view.rs

Handles the `view` command which provides a detailed view of a specific COMTRADE file.

Arguments for the view command.
* `file` - The path to the COMTRADE file to be viewed. Es need only DAT file (.dat). 

* Print detailed information about the specified COMTRADE file.
* Print detailed information from .dat file 
* Print line plots for the analog channels using terminal plotting library. 
* Print status plots for the binary channels using terminal plotting library. 