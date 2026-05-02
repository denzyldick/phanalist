<?php

class Isolated {
    private $a;
    private $b;

    public function getA() {
        return new class($this->a) {
            private $val;
            public function __construct($v) { $this->val = $v; }
            public function run() { return $this->val; }
        };
    }

    public function getB() {
        // This uses $b, but it's disconnected from getA's usage of $a
        return $this->b;
    }
}
