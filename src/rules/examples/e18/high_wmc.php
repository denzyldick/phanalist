<?php

namespace Test\e18;

class HighWmc
{
    public function method1(): void
    {
        if (true) {
            if (false) {
                while (true) {
                    if (1 === 1) {}
                }
            }
        }
        foreach ([] as $item) {
            if ($item) {}
        }
    }

    public function method2(): void
    {
        switch (1) {
            case 1:
                if (true) {}
                break;
            case 2:
                if (false) {}
                break;
            case 3:
                break;
        }
        if (true) {
            if (false) {}
        }
    }

    public function method3(): void
    {
        for ($i = 0; $i < 10; $i++) {
            if ($i > 5) {
                if ($i > 8) {}
            }
        }
        while (true) {
            if (true) { if (false) {} }
        }
    }

    public function method4(): void
    {
        if (true) { if (true) { if (true) { if (true) {} } } }
        foreach ([] as $v) { if ($v) {} }
    }

    public function method5(): void
    {
        try {
            if (true) {}
        } catch (\Exception $e) {
            if (false) {}
        }
        if (true) { if (true) {} }
        while (true) {}
    }

    public function method6(): void
    {
        if (1) { if (2) { if (3) {} } }
        switch (1) { case 1: case 2: case 3: }
    }

    public function method7(): void
    {
        if (true) {}
        if (true) {}
        if (true) {}
        if (true) {}
        if (true) {}
    }

    public function method8(): void
    {
        foreach ([] as $a) {
            foreach ([] as $b) {
                if ($a && $b) {}
            }
        }
    }
}
