// Package docs is the setup of the pages in this directory: the runners of their cases
// tables, and what their fact expressions use by name.
package docs

import (
	"fmt"
	"sort"
	"strconv"
	"strings"

	"example.com/shop"
	"example.com/shop/shipping"
)

// Cases are the runners of cases tables, by name.
var Cases = map[string]func(row map[string]string) error{
	"discount": discount,
	"delivery": delivery,
	"total":    total,
}

// Columns are the columns each runner's table must have.
var Columns = map[string][]string{
	"discount": {"price", "percent", "result"},
	"total":    {"basket", "region", "total"},
}

func discount(row map[string]string) error {
	price, _ := strconv.Atoi(row["price"])
	percent, _ := strconv.Atoi(row["percent"])
	got, err := shop.ApplyDiscount(shop.EUR(price), percent)
	if err != nil {
		return err
	}
	if strconv.Itoa(got.Cents) != row["result"] {
		return fmt.Errorf("expected %s, got %d", row["result"], got.Cents)
	}
	return nil
}

// total holds a row of a basket's prices, in cents, and its total with the tax of a
// region. A price that is not a number panics, as a runner with a bug would: the broken
// page shows what docattest makes of that.
func total(row map[string]string) error {
	basket := []shop.Money{}
	for _, field := range strings.Fields(row["basket"]) {
		cents, err := strconv.Atoi(field)
		if err != nil {
			panic(err)
		}
		basket = append(basket, shop.EUR(cents))
	}
	got, err := shop.Total(basket, row["region"])
	if err != nil {
		return err
	}
	if strconv.Itoa(got.Cents) != row["total"] {
		return fmt.Errorf("expected %s, got %d", row["total"], got.Cents)
	}
	return nil
}

func delivery(row map[string]string) error {
	got, err := shipping.Quote(row["region"])
	if err != nil {
		return err
	}
	if strconv.Itoa(got) != row["days"] {
		return fmt.Errorf("expected %s days, got %d", row["days"], got)
	}
	return nil
}

// ErrorMessages are the messages of the shop's errors.
func ErrorMessages() []string {
	messages := []string{}
	for _, message := range shop.ErrorCodes {
		messages = append(messages, message)
	}
	sort.Strings(messages)
	return messages
}

// Rates are the tax rates as a page writes them, for the regions given.
func Rates(regions ...string) []string {
	rates := []string{}
	for _, region := range regions {
		rates = append(rates, fmt.Sprintf("%d%%", shop.TaxRates[region]))
	}
	return rates
}

// Codes are the codes of the regions in a snapshot of the upstream list.
func Codes(regions any) []string {
	codes := []string{}
	for _, region := range regions.([]any) {
		codes = append(codes, fmt.Sprint(region.(map[string]any)["code"]))
	}
	return codes
}
