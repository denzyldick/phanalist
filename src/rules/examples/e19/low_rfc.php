<?php

namespace Test\e19;

class LowRfc
{
    public function getName(): string
    {
        return strtolower($this->format());
    }

    public function format(): string
    {
        return trim('hello');
    }
}
