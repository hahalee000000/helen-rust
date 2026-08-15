# Tutorial Test System — Initial Report

## Summary

Successfully created a test system that extracts and validates 235 code blocks from Helen tutorial documentation.

**Test Results:**
- ✅ **98 tests pass** (41.7%)
- ✅ **10 tests correctly fail** (intentional error examples)
- ⚠️ **107 tests fail** (tutorial issues identified)
- ⏭️ **19 tests skipped** (require external resources)
- 🔄 **1 unexpected pass** (should fail but passes)

## Files Created

1. **`wiki/reference/generate_tests.py`** (273 lines)
   - Extracts `helen` code blocks from markdown
   - Classifies blocks (complete/fragment/error/skip)
   - Auto-prepends context for fragments
   - Generates 235 test files in `wiki/reference/tests/`

2. **`wiki/reference/run_tests.sh`** (80 lines)
   - Runs `helen check` on all test files
   - Interprets results based on metadata markers
   - Reports color-coded summary

3. **`wiki/reference/tests/README.md`**
   - Complete documentation of the test system
   - Usage instructions
   - Known issues catalog

4. **`wiki/reference/tests/`** (235 test files)
   - 17 subdirectories (one per tutorial file)
   - Each file links back to source (file:line)
   - Metadata: `@should_fail`, `@skip: <reason>`

## Key Findings

### 1. Return Type Syntax (Critical — Affects Many Files)

**Issue:** Tutorial uses `->` for return types, but Helen uses `:`

**Example:**
```helen
// Tutorial (WRONG):
fn greet(name: string) -> string {
    return "Hello, " + name + "!"
}

// Helen (CORRECT):
fn greet(name: string): string {
    return "Hello, " + name + "!"
}
```

**Affected:** `01-getting-started.md`, `03-functions.md`, and others
**Impact:** ~15 test failures

### 2. Undefined References (High Frequency)

**Issue:** Many fragments reference undefined functions/variables

**Examples:**
- `expensiveFunction()`, `getUser()`, `isValid()` in `04-control-flow.md`
- `fix_code()` in `11-building-agents.md`, `14-observability.md`
- `stream_print()` in `06-llm-statements.md`（流式回调已通过 `on_chunk` 处理）

**Impact:** ~40 test failures

### 3. Duplicate Declarations (Context Accumulation Issue)

**Issue:** When blocks redefine variables from context, causes duplicate declaration errors

**Example:**
```helen
main {
    // Context from previous block
    let a = [1, 2]

    // Current block
    let a = [3, 4]  // Error: duplicate declaration

}```

**Impact:** ~10 test failures

### 4. Import Dependencies

**Issue:** Many examples require external files or Python modules

**Examples:**
- File imports: `import "./utils.helen"` (needs actual file)
- Python FFI: `import "math"`, `import "json"` (needs Python modules)

**Impact:** 19 tests skipped

### 5. Type Checking Gap

**Issue:** Tutorial claims type mismatches should fail, but Helen's semantic analyzer doesn't enforce strict type checking

**Example:**
```helen
main {
    fn add(a: int, b: int): int { return a + b }
    let x = 1.5
    add(x, 2)  // Tutorial says: runtime error
               // Reality: passes helen check

}```

**Impact:** 1 unexpected pass

## Recommendations

### Immediate Fixes (High Impact)

1. **Fix return type syntax** — Replace all `->` with `:` in tutorials
   - Files: `01-getting-started.md`, `03-functions.md`, others
   - Effort: Low (simple find/replace)
   - Impact: Fixes ~15 tests

2. **Define placeholder functions** — Add stub definitions for undefined functions
   - Files: `04-control-flow.md`, `11-building-agents.md`, etc.
   - Effort: Medium (need to define ~20 functions)
   - Impact: Fixes ~40 tests

3. **Improve context handling** — Detect variable redefinitions in generator
   - File: `generate_tests.py`
   - Effort: Medium (add redefinition detection)
   - Impact: Fixes ~10 tests

### Medium-Term Improvements

4. **Create import stubs** — Generate stub files for import examples
   - Effort: Medium (create ~10 stub files)
   - Impact: Enables testing of import examples

5. **Mark syntax templates** — Add `@skip: syntax template` to non-runnable examples
   - Effort: Low (identify and mark ~5 blocks)
   - Impact: Improves test accuracy

### Long-Term Enhancements

6. **Execution testing** — Run tests with `helen` (not just `helen check`)
   - Effort: High (need to mock LLM, handle imports, etc.)
   - Impact: Validates runtime behavior

7. **Output validation** — Compare actual output with tutorial claims
   - Effort: High (need output parsing, comparison logic)
   - Impact: Validates tutorial correctness

## Next Steps

1. ✅ Test system created and working
2. ⏳ Fix return type syntax in tutorials
3. ⏳ Add missing function definitions
4. ⏳ Improve generator context handling
5. ⏳ Create import stubs
6. ⏳ Re-run tests to verify improvements

## Usage

```bash
# Generate tests
python wiki/reference/generate_tests.py

# Run tests
bash wiki/reference/run_tests.sh

# View results
bash wiki/reference/run_tests.sh | tee tutorial_test_results.txt
```

## Conclusion

The test system successfully identifies tutorial issues and validates language implementation. The 41.7% pass rate reflects both genuine tutorial errors and limitations in the test approach (context handling, missing definitions). With the recommended fixes, pass rate should improve to ~70-80%.

**The system achieves its dual goals:**
1. ✅ Tests Helen language implementation (syntax + semantics)
2. ✅ Validates tutorial correctness (identifies errors)

---

Generated: 2026-07-09
Total test files: 235
Total tutorial files: 17
