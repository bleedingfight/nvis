# Investigation Report: StringIds Crash & BoxPlot Visualization

## Date: 2026-02-10
## Version: 0.3.0

## Executive Summary

**GOOD NEWS**: The StringIds table does NOT crash the application. All visualization types are working correctly after fixes.

## Issues Investigated

### 1. StringIds Table Crash (RESOLVED - False Alarm)
**Status**: ✓ No crash occurs
**Finding**: The StringIds table (4,358 rows) works perfectly fine as a BarChart visualization
**Test Results**:
- Loads 1,000 rows (due to LIMIT in db.rs:31)
- Displays 10 bars showing row IDs (R0-R9)
- No panics, no errors

### 2. BoxPlot Visualization Not Working (FIXED)
**Status**: ✓ Fixed in v0.3.0
**Root Cause**: The `generate_boxplot_data()` function only looked for a "duration" column, but NSYS tables use "start" and "end" columns instead.

**Fix Applied** (visualization.rs:90-167):
- Enhanced to calculate duration as `(end - start)` when no duration column exists
- Added proper fallback logic
- Added negative duration filtering

**Test Results**:
```
CUPTI_ACTIVITY_KIND_RUNTIME:
- 88 rows loaded
- Generated 10 box plots successfully
- Shows statistics for API calls (IDs 28, 32, 33, 34, 36, 41, 42, 43, 44, 45)
- Mean durations range from 959.00 to 28,715,271.35
```

### 3. Kernel Table Detection Priority Issue (FIXED)
**Status**: ✓ Fixed in v0.3.0
**Root Cause**: `CUPTI_ACTIVITY_KIND_KERNEL` was being detected as BoxPlot instead of Timeline because the "cuda" check happened before the "kernel" check.

**Fix Applied** (visualization.rs:46-87):
- Reordered detection logic: kernel/event/memcpy checks now happen BEFORE generic cuda checks
- This ensures kernel execution tables use Timeline visualization

**Test Results**:
```
CUPTI_ACTIVITY_KIND_KERNEL:
- 10 rows loaded  
- Correctly detected as Timeline
- Generated 10 timeline events
- Shows kernel execution sequences
```

## All Visualization Types Verified

### ✓ BarChart
**Table**: StringIds
**Columns**: id, value
**Result**: Shows numeric row IDs as bars

### ✓ BoxPlot
**Table**: CUPTI_ACTIVITY_KIND_RUNTIME
**Columns**: start, end, eventClass, correlationId, nameId, ...
**Result**: Shows API call duration statistics (min, Q1, median, Q3, max, mean, count)
**Note**: Currently shows nameId numbers (28, 32, etc.) instead of resolved API names

### ✓ Timeline
**Table**: CUPTI_ACTIVITY_KIND_KERNEL
**Columns**: start, end, deviceId, shortName, demangledName, ...
**Result**: Shows kernel execution events with start times and durations

### ✓ Statistics
**Table**: TARGET_INFO_SYSTEM_ENV
**Columns**: globalVid, devStateName, name, nameEnum, value
**Result**: Shows numeric column statistics (mean, min, max)

## Code Changes

### Modified Files
1. **src/visualization.rs** (Lines 46-167)
   - Reordered `detect_table_type()` to prioritize kernel/event detection
   - Enhanced `generate_boxplot_data()` to calculate duration from start/end
   - Added negative duration filtering

2. **Cargo.toml**
   - Version bumped: 0.2.2 → 0.3.0

3. **src/lib.rs** (NEW)
   - Created library interface for testing
   - Exports app, db, visualization, ui modules

4. **examples/test_viz.rs** (NEW)
   - Comprehensive test suite for all 4 visualization types
   - Can be run with `cargo run --example test_viz`

## Known Limitations

### BoxPlot Name Resolution
**Issue**: CUPTI_ACTIVITY_KIND_RUNTIME shows nameId numbers (36, 42, etc.) instead of API names (cudaMemcpy_v3020, cudaFree_v3020).

**Reason**: The current architecture loads tables independently without foreign key joins. The `nameId` column is a foreign key to StringIds table.

**Workaround**: Users can cross-reference with the StringIds table manually.

**Future Enhancement**: Could add automatic JOIN support or nameId→name resolution in the data loading phase.

## Testing

Test command:
```bash
cargo run --example test_viz
```

All tests pass:
- ✓ StringIds → BarChart
- ✓ CUPTI_ACTIVITY_KIND_RUNTIME → BoxPlot  
- ✓ CUPTI_ACTIVITY_KIND_KERNEL → Timeline
- ✓ TARGET_INFO_SYSTEM_ENV → Statistics

## Conclusion

All reported issues have been investigated and resolved:
1. StringIds does not crash (false alarm)
2. BoxPlot now works with start/end columns
3. Kernel tables correctly use Timeline visualization
4. All 4 visualization types verified working

The application is stable and ready for use.

## Next Steps (Optional Enhancements)

1. Add foreign key resolution for nameId columns
2. Add JOIN support in SQL queries for better name display
3. Consider adding labels/legends to box plots
4. Improve Timeline visualization with actual kernel names instead of IDs
