<?php

namespace Test\e11;

class SetLocalInMethod {

    private int $counter = 0;

    public function increaseCounter(int $counter): int {
        $increased = $counter + 1;

        return $increased;
    }
}