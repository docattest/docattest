// Package shipping says who delivers, and how fast.
package shipping

import "fmt"

// Carriers are the carriers that deliver.
var Carriers = []string{"post", "courier", "drone"}

// days are the delivery times in days, by region.
var days = map[string]int{"NL": 1, "DE": 2}

// unserved is the error of a region no carrier serves.
const unserved = "no carrier serves %q"

// Quote is the number of days a delivery to a region takes.
func Quote(region string) (int, error) {
	found, served := days[region]
	if !served {
		return 0, fmt.Errorf(unserved, region)
	}
	return found, nil
}
