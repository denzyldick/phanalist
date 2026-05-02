<?php

class Complex {
    public function veryComplex($a, $b, $c) {
        if ($a) { // +1
            if ($b) { // +2 (nesting 1)
                while ($c) { // +3 (nesting 2)
                    if ($a && $b) { // +4 (nesting 3) + 1 (boolean)
                        echo "foo";
                    }
                }
            } else if ($c) { // +1 (else if)
                echo "bar";
            } else { // +1 (else)
                echo "baz";
            }
        }
        
        return $a ? $b : $c; // +1 (ternary)
    }
}
