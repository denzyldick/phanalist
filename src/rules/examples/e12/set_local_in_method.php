<?php

namespace App\Service\e12;

class SetLocalInMethod {

    private int $counter = 0;

    public function increaseCounter(int $counter): int {
        $increased = $counter + 1;

        return $increased;
    }
}