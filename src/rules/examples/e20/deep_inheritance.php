<?php

namespace Test\e20;

class A {}
class B extends A {}
class C extends B {}
class D extends C {}
class E extends D {}  // depth = 4 → violation at default threshold 4
class F extends E {}  // depth = 5 → violation
