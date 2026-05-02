<?php

class NonCohesive {
    private $a;
    private $x;

    public function methodA() {
        return $this->a;
    }

    public function methodX() {
        return $this->x;
    }
}
