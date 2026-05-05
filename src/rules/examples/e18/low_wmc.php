<?php

namespace Test\e18;

class LowWmc
{
    public function getName(): string
    {
        return 'hello';
    }

    public function getAge(): int
    {
        return 42;
    }

    public function isActive(): bool
    {
        if ($this->active) {
            return true;
        }
        return false;
    }
}
