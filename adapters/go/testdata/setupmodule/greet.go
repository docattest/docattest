// Package greet says hello.
package greet

// Greeting is the word every greeting starts with.
const Greeting = "Hello"

// Hello greets someone by name.
func Hello(name string) string { return Greeting + ", " + name + "!" }

// Wave is what no page attests.
func Wave() string { return "o/" }
