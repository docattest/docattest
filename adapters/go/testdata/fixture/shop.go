// Package shop prices baskets: money, discounts and tax.
package shop

import (
	"errors"
	"fmt"
)

// DefaultCurrency is the currency of a price that names none.
const DefaultCurrency = "EUR"

// MaxDiscount is the largest discount, in percent.
const MaxDiscount = 50

// TaxRates are the tax rates in percent, by region.
var TaxRates = map[string]int{"NL": 21, "DE": 19, "FR": 20}

// ErrorCodes are the messages of the errors this package returns, by code.
var ErrorCodes = map[string]string{
	"out_of_stock": "The item is not available",
	"bad_discount": "The discount is outside the allowed range",
	"no_carrier":   "No carrier serves the region",
}

// ErrBadDiscount is returned for a discount below zero or above MaxDiscount.
var ErrBadDiscount = errors.New(ErrorCodes["bad_discount"])

// Money is an amount in the cents of a currency.
type Money struct {
	Cents    int
	Currency string
}

// EUR is an amount of cents in the default currency.
func EUR(cents int) Money {
	return Money{Cents: cents, Currency: DefaultCurrency}
}

// String writes an amount as a reader expects it: "7.50 EUR".
func (m Money) String() string {
	return fmt.Sprintf("%d.%02d %s", m.Cents/100, m.Cents%100, m.Currency)
}

// ApplyDiscount takes a percentage off a price, rounding half up to a whole cent.
func ApplyDiscount(price Money, percent int) (Money, error) {
	if percent < 0 || percent > MaxDiscount {
		return Money{}, ErrBadDiscount
	}
	price.Cents = round(price.Cents * (100 - percent))
	return price, nil
}

// Total adds up a basket and the tax of a region.
func Total(basket []Money, region string) (Money, error) {
	rate, known := TaxRates[region]
	if !known {
		return Money{}, fmt.Errorf("no tax rate for region %q", region)
	}
	sum := 0
	for _, price := range basket {
		sum += price.Cents
	}
	return EUR(round(sum * (100 + rate))), nil
}

// round divides hundredths of a cent by a hundred, half up.
func round(hundredths int) int {
	return (hundredths + 50) / 100
}
