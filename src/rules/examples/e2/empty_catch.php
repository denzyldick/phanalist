<?php

namespace Test\e2;

class EmptyCatch {
    public function test() {
        try {
            $this->throw();
        } catch(Exception $e) {}
    }

    public function throw() {
        throw new Exception("test");
    }
}