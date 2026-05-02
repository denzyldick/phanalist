<?php

class Cohesive {
    private $a;
    private $b;

    public function setA($a) {
        $this->a = $a;
    }

    public function setB($b) {
        $this->b = $b;
    }

    public function process() {
        return $this->a + $this->b;
    }
}
