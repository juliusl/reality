
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
<terminal/builtin.println> Adding terminal project

# -- Test argument 
: .label placeholder

# -- Adds a new desktop app project
<desktop/builtin.println> Adding desktop project

# -- Test argument 
: .label placeholder
```