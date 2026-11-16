// Package shipping says who delivers, and how fast.
package shipping

import "fmt"

// Carriers are the carriers that deliver.
var Carriers = []string{"post", "courier", "drone"}

// days are the delivery times in days, by region.
var days = map[string]int{"NL": 1, "DE": 2}

// Quote is the number of days a delivery to a region takes.
func Quote(region string) (int, error) {
	found, served := days[region]
	if !served {
		return 0, fmt.Errorf("no carrier serves %q", region)
	}
	return found, nil
}
