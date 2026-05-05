<?php

namespace Test\e20;

interface Printable {}
interface Loggable {}

class MyClass implements Printable, Loggable
{
    public function doSomething(): void {}
}
