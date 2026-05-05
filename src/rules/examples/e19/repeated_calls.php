<?php

namespace Test\e19;

class RepeatedCalls
{
    public function process(): void
    {
        $this->log();
        $this->log();
        $this->log();
        $this->log();
        $this->log();
        strlen('a');
        strlen('b');
        strlen('c');
    }

    private function log(): void
    {
    }
}
