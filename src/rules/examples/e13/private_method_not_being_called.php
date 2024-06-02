<?php

namespace DeadCode {

  class Test {

    private function isNotCalled(): bool {

      $this->testHelloworld();
      $this->testHelloworld();
      return true;
    }


    private function testing() {
    }

    private function helloworld() {
    }
    public function testing2() {
    }
    public function test() {
    }
    private function testHelloworld() {
    }
  }
}
