package program

// docattestFacts serves fact expressions: each item is the Observation of one.
func docattestFacts(facts []func() any) {
	docattestServe(docattestStart(), len(facts), func(k int) any {
		return docattestObserve(facts[k]())
	})
}
