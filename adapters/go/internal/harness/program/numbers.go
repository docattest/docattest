package program

import (
	"encoding/json"
	"fmt"
)

// DocattestNumbers is JSON that encoding/json read into an `any` with UseNumber, each
// number as Go holds a value written so: an int64 when it is written as an integer that
// one holds, as a constant or a typed field holds it, and any other a float64, as
// encoding/json reads every number. Read as a float64, 1048576 would print as
// 1.048576e+06. Maps and slices are changed in place. The adapter shows the values of
// data blocks by it too, so that a value of a block, of a snapshot and of a manifest
// prints alike.
func DocattestNumbers(value any) (any, error) {
	switch read := value.(type) {
	case json.Number:
		return docattestNumber(read)
	case map[string]any:
		for key, item := range read {
			number, err := DocattestNumbers(item)
			if err != nil {
				return nil, err
			}
			read[key] = number
		}
	case []any:
		for k, item := range read {
			number, err := DocattestNumbers(item)
			if err != nil {
				return nil, err
			}
			read[k] = number
		}
	}
	return value, nil
}

// docattestNumber is one number as DocattestNumbers reads it; an error for one that no
// float64 holds, which encoding/json refuses too.
func docattestNumber(number json.Number) (any, error) {
	if integer, err := number.Int64(); err == nil {
		return integer, nil
	}
	float, err := number.Float64()
	if err != nil {
		return nil, fmt.Errorf("the number %s is out of range: %w", number, err)
	}
	return float, nil
}
