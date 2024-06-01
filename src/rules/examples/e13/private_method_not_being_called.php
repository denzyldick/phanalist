<?php

namespace DeadCode {

  class Test {

    private function isNotCalled(): bool {

      $this->testHelloworld();
      $this->testHelloworld();
      return true;
    }


    public function test() {
    }
    private function testHelloworld() {
    }
  }
}
