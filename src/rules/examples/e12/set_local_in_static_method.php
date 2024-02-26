<?php

namespace App\Service\e12;

class SetLocalInStaticMethod {

    private static int $counter = 0;

    public static function setCounter(int $counter): void {
        $new = $counter;
    }
}