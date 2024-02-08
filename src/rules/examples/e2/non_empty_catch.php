<?php

namespace Test\e2;

class NonEmptyCatch {
    public function test() {
        try {
            $this->throw();
        } catch(Exception $e) {
            $this->log($e->getMessage());
        }
    }

    public function throw() {
        throw new Exception("test");
    }

    public function log(string $message) {
    }
}