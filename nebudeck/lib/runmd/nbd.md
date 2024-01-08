```runmd
# -- Commands for installing dependencies
+ .operation install-deps

# -- Install nebudeck as a dependency of the current project
<nebudeck/builtin.process> cargo
: .arg  install
: .arg  nebudeck

# -- Install loopio as a dependency of the current project
<loopio/builtin.process> cargo
: .arg  install
: .arg  loopio

# -- Adds a new nebudeck project
+ .operation add-project

# -- Adds a new terminal app project
<terminal/nebudeck.project> terminal
|# arg.name = new_terminal_project

# -- Adds a new desktop app project
<desktop/nebudeck.project> desktop
|# arg.name = new_desktop_project

# -- Title of the desktop app window
: .title  New Desktop App

# -- Initial window height
: .height 1920.0

# -- Initial window width
: .width  1080.0

```
