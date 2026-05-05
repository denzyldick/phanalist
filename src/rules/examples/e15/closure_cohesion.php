<?php

class ClosureCohesion {
    private $a;

    public function methodA() {
        return array_map(fn() => $this->a, [1, 2]);
    }

    public function methodB() {
        return function() {
            return $this->a;
        };
    }
}
