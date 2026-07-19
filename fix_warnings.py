import re

with open('src/json_storage.rs', 'r') as f:
    content = f.read()

content = re.sub(r'let commands_clone =', 'let _commands_clone =', content)
content = re.sub(r'let binds_clone =', 'let _binds_clone =', content)
content = re.sub(r'let suggestions_clone =', 'let _suggestions_clone =', content)
content = re.sub(r'let \(id, commands_clone\) =', 'let (id, _commands_clone) =', content)
content = re.sub(r'let \(id, binds_clone\) =', 'let (id, _binds_clone) =', content)
content = re.sub(r'let \(id, suggestions_clone\) =', 'let (id, _suggestions_clone) =', content)

with open('src/json_storage.rs', 'w') as f:
    f.write(content)

