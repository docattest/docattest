package shop

import (
	"errors"
	"fmt"
	"testing"
)

// A discount is taken off the price in whole cents.
func ExampleApplyDiscount() {
	price, _ := ApplyDiscount(EUR(999), 50)
	fmt.Println(price)
	// Output: 5.00 EUR
}

// This example states no output, so it is compiled and never run.
func ExampleTotal() {
	_, _ = Total([]Money{EUR(1000)}, "NL")
}

func TestTotals(t *testing.T) {
	t.Run("adds tax", func(t *testing.T) {
		got, err := Total([]Money{EUR(1000)}, "NL")
		if err != nil || got != EUR(1210) {
			t.Fatalf("got %v, %v", got, err)
		}
	})
	t.Run("rejects big discounts", func(t *testing.T) {
		if _, err := ApplyDiscount(EUR(1000), 90); !errors.Is(err, ErrBadDiscount) {
			t.Fatalf("got %v", err)
		}
	})
}

func BenchmarkTotal(b *testing.B) {
	for i := 0; i < b.N; i++ {
		_, _ = Total([]Money{EUR(1000)}, "NL")
	}
}
