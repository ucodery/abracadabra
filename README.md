# abracadabra
make development environments magically appear

Abracadabra is a tool that can be run from any directory in order to prepare the current environment and location for coding.

## Design Goals

1. Isolation
   is a good thing and every project will be set up independently of all others
2. Not a universal packaging system
   Abracadabra only take care of first time setup. It does not try to install everything necessary to run development tasks,
   only to make the tools expected by that project available
