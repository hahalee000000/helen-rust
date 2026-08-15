# Automated Compliance Checking Pattern

When verifying that code matches design documents, use this Python script pattern. It systematically checks:
1. Class/type names match
2. Method signatures match
3. Key functionality is implemented

## Usage

```python
"""Check Phase X code vs design docs"""
import re

def read_file(path):
    with open(path, 'r', encoding='utf-8') as f:
        return f.read()

# Read design docs and code files
design_doc = read_file('/path/to/design.md')
code_file = read_file('/path/to/code.py')

# Extract expected items from design doc
expected_classes = set(re.findall(r'class\s+(\w+)\b', design_doc))
expected_methods = set(re.findall(r'def\s+(\w+)\b', design_doc))

# Extract actual items from code
actual_classes = set(re.findall(r'class\s+(\w+)\b', code_file))
actual_methods = set(re.findall(r'def\s+(\w+)\b', code_file))

# Check compliance
missing_classes = expected_classes - actual_classes
missing_methods = expected_methods - actual_methods
extra_classes = actual_classes - expected_classes
extra_methods = actual_methods - expected_methods

# Report
if not missing_classes and not missing_methods:
    print("✅ 100% compliant")
else:
    if missing_classes: print(f"❌ Missing classes: {missing_classes}")
    if missing_methods: print(f"❌ Missing methods: {missing_methods}")
```

## Key Patterns for Hellen Language

### TokenType Check
```python
design_tokens = set(re.findall(r'(\w+)\s*=\s*auto\(\)', design_doc))
code_tokens = set(re.findall(r'(\w+)\s*=\s*auto\(\)', code_file))
```

### AST Node Check
```python
design_nodes = set(re.findall(r'class\s+(\w+Node)\b', design_doc))
code_nodes = set(re.findall(r'class\s+(\w+Node)\b', code_file))
```

### Error Code Check
```python
design_errors = {m.group(1): int(m.group(2)) 
                 for m in re.finditer(r'(\w+)\s*=\s*(\d{3})', design_doc)
                 if 300 <= int(m.group(2)) <= 314}
```

### Visitor Method Check
```python
expected_visits = set()
for node in code_nodes:
    parts = re.findall(r'[A-Z][a-z]*', node)
    if parts:
        visit_name = 'visit_' + '_'.join(p.lower() for p in parts)
        expected_visits.add(visit_name)

actual_visits = set(re.findall(r'def\s+(visit_\w+)\(', code_file))
```
