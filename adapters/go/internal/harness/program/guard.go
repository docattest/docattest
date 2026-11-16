// Package program is the part of a generated program that is the same every time. See
// the harness package, which embeds these files as text.
package program

import (
	docattestcontext "context"
	docattesterrors "errors"
	docattestnet "net"
)

// docattestRefuseNetwork makes every lookup of a host name fail, which is what reaching
// the internet begins with. Names in the hosts file (localhost) still resolve, and an
// address written as numbers is dialled as before.
func docattestRefuseNetwork() {
	refuse := func(docattestcontext.Context, string, string) (docattestnet.Conn, error) {
		return nil, docattesterrors.New("docattest: the network is off; tag the block `check network`")
	}
	docattestnet.DefaultResolver = &docattestnet.Resolver{PreferGo: true, Dial: refuse}
}
