<?php

namespace Test\e22;

class ServiceA
{
    public function doSomething(ServiceB $b): void {}
}

class ServiceB
{
    public function doSomethingElse(ServiceA $a): void {}
}
